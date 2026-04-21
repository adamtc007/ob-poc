//! `research.generic.normalize` — SemOS-side YAML-first re-implementation.
//!
//! Canonicalises a raw research payload (recursive key sort + string
//! trim) and produces a SHA-256 content hash. Upserts into
//! `"ob-poc".research_normalized_payloads` on the hash for dedup.
//! Write runs in the Sequencer-owned scope.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string, json_extract_string_opt};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

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

pub struct Normalize;

#[async_trait]
impl SemOsVerbOp for Normalize {
    fn fqn(&self) -> &str {
        "research.generic.normalize"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
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
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_sorts_keys() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"b": 1, "a": 2, "c": 3}"#).unwrap();
        let canonical = canonicalize_json(&input);
        assert_eq!(
            serde_json::to_string(&canonical).unwrap(),
            r#"{"a":2,"b":1,"c":3}"#
        );
    }

    #[test]
    fn canonicalize_trims_strings() {
        let input: serde_json::Value =
            serde_json::from_str(r#"{"name": "  hello  ", "city": " London "}"#).unwrap();
        let canonical = canonicalize_json(&input);
        assert_eq!(canonical["name"], serde_json::Value::String("hello".to_string()));
        assert_eq!(canonical["city"], serde_json::Value::String("London".to_string()));
    }

    #[test]
    fn same_payload_same_hash() {
        let json_a = r#"{"name": "Allianz", "country": "DE", "lei": "ABC123"}"#;
        let json_b = r#"{"lei": "ABC123", "name": "Allianz", "country": "DE"}"#;

        let ca = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_a).unwrap());
        let cb = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_b).unwrap());

        let sa = serde_json::to_string(&ca).unwrap();
        let sb = serde_json::to_string(&cb).unwrap();
        assert_eq!(sa, sb);

        let ha = {
            let mut h = Sha256::new();
            h.update(sa.as_bytes());
            hex::encode(h.finalize())
        };
        let hb = {
            let mut h = Sha256::new();
            h.update(sb.as_bytes());
            hex::encode(h.finalize())
        };
        assert_eq!(ha, hb);
    }

    #[test]
    fn different_payload_different_hash() {
        let json_a = r#"{"name": "Allianz"}"#;
        let json_b = r#"{"name": "BlackRock"}"#;
        let ca = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_a).unwrap());
        let cb = canonicalize_json(&serde_json::from_str::<serde_json::Value>(json_b).unwrap());
        let ha = {
            let mut h = Sha256::new();
            h.update(serde_json::to_string(&ca).unwrap().as_bytes());
            hex::encode(h.finalize())
        };
        let hb = {
            let mut h = Sha256::new();
            h.update(serde_json::to_string(&cb).unwrap().as_bytes());
            hex::encode(h.finalize())
        };
        assert_ne!(ha, hb);
    }
}
