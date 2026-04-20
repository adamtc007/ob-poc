//! Research Generic Normalize Operations.
//!
//! Canonicalises raw research payloads (sorted keys, trimmed strings) and
//! produces a SHA-256 content hash for deduplication, optionally persisting
//! to the `research_normalized_payloads` table.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string, json_extract_string_opt};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizeResult {
    pub source_name: String,
    pub canonical_json: serde_json::Value,
    pub content_hash: String,
    pub field_count: usize,
    pub normalized_at: String,
}

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

#[register_custom_op]
pub struct ResearchGenericNormalizeOp;

fn normalize_payload_impl(source_name: String, payload_raw: &str) -> Result<NormalizeResult> {
    let parsed: serde_json::Value = serde_json::from_str(payload_raw)
        .map_err(|e| anyhow::anyhow!("Invalid JSON payload: {}", e))?;
    let canonical = canonicalize_json(&parsed);
    let canonical_str = serde_json::to_string(&canonical)?;

    let mut hasher = Sha256::new();
    hasher.update(canonical_str.as_bytes());
    let content_hash = hex::encode(hasher.finalize());

    let field_count = match &canonical {
        serde_json::Value::Object(map) => map.len(),
        _ => 0,
    };

    Ok(NormalizeResult {
        source_name,
        canonical_json: canonical,
        content_hash,
        field_count,
        normalized_at: chrono::Utc::now().to_rfc3339(),
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

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
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

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

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
        assert_eq!(serde_json::to_string(&canonical).unwrap(), r#"{"a":2,"b":1,"c":3}"#);
    }

    #[test]
    fn test_canonicalize_trims_strings() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"name": "  hello  ", "city": " London "}"#).unwrap();
        let canonical = canonicalize_json(&input);
        assert_eq!(canonical["name"], serde_json::Value::String("hello".to_string()));
        assert_eq!(canonical["city"], serde_json::Value::String("London".to_string()));
    }

    #[test]
    fn test_same_payload_same_hash() {
        let json_a = r#"{"name": "Allianz", "country": "DE", "lei": "ABC123"}"#;
        let json_b = r#"{"lei": "ABC123", "name": "Allianz", "country": "DE"}"#;

        let canonical_a = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_a).unwrap());
        let canonical_b = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_b).unwrap());

        let str_a = serde_json::to_string(&canonical_a).unwrap();
        let str_b = serde_json::to_string(&canonical_b).unwrap();

        assert_eq!(str_a, str_b);

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

        let canonical_a = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_a).unwrap());
        let canonical_b = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_b).unwrap());

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
