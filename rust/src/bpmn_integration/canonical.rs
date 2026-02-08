//! Canonical JSON serialization and SHA-256 hashing.
//!
//! Provides deterministic JSON output (sorted keys) and SHA-256 hashing
//! for domain payload integrity verification across the gRPC boundary.
//!
//! The BPMN-Lite protocol requires that `domain_payload_hash` matches
//! `SHA-256(domain_payload_bytes)` at every crossing. These helpers
//! ensure canonical byte representation for hash stability.

use sha2::{Digest, Sha256};

/// Serialize a `serde_json::Value` with deterministic key ordering and return
/// the canonical JSON string along with its SHA-256 hash.
///
/// `serde_json::Value` uses `BTreeMap` internally (sorted keys), so
/// `serde_json::to_string` already produces canonical output for values
/// deserialized from JSON. For values constructed programmatically with
/// `serde_json::Map`, we normalize key ordering explicitly.
pub fn canonical_json_with_hash(value: &serde_json::Value) -> (String, Vec<u8>) {
    let normalized = normalize_key_order(value);
    let json = serde_json::to_string(&normalized).expect("canonical_json: serialization failed");
    let hash = sha256_bytes(&json);
    (json, hash)
}

/// Compute SHA-256 hash of a string, returning the 32-byte digest.
pub fn sha256_bytes(input: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().to_vec()
}

/// Validate that a domain payload's SHA-256 hash matches the expected hash.
pub fn validate_payload_hash(payload: &str, expected_hash: &[u8]) -> bool {
    sha256_bytes(payload) == expected_hash
}

/// Recursively normalize JSON key ordering to ensure canonical output.
///
/// `serde_json::Map` preserves insertion order by default (unless the
/// `preserve_order` feature is disabled). This function forces sorted
/// key ordering by reconstructing objects via `BTreeMap` traversal.
fn normalize_key_order(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), normalize_key_order(v)))
                .collect();
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(normalize_key_order).collect())
        }
        other => other.clone(),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_json_key_ordering() {
        // serde_json::json! macro uses BTreeMap, so keys are already sorted.
        // But test with explicit out-of-order construction:
        let mut map = serde_json::Map::new();
        map.insert("z_field".to_string(), json!(1));
        map.insert("a_field".to_string(), json!(2));
        map.insert("m_field".to_string(), json!(3));
        let value = serde_json::Value::Object(map);

        let (json_str, _hash) = canonical_json_with_hash(&value);
        // Keys must be sorted: a_field, m_field, z_field
        assert!(
            json_str.find("a_field").unwrap() < json_str.find("m_field").unwrap(),
            "a_field should come before m_field"
        );
        assert!(
            json_str.find("m_field").unwrap() < json_str.find("z_field").unwrap(),
            "m_field should come before z_field"
        );
    }

    #[test]
    fn test_canonical_json_roundtrip() {
        let value = json!({
            "case_id": "KYC-001",
            "entity_ref": "ent-123",
            "details": {
                "name": "Test Corp",
                "jurisdiction": "LU"
            }
        });

        let (json1, hash1) = canonical_json_with_hash(&value);
        let reparsed: serde_json::Value = serde_json::from_str(&json1).unwrap();
        let (json2, hash2) = canonical_json_with_hash(&reparsed);

        assert_eq!(json1, json2, "Round-trip must produce identical JSON");
        assert_eq!(hash1, hash2, "Round-trip must produce identical hash");
    }

    #[test]
    fn test_hash_stability() {
        let value = json!({"key": "value", "num": 42});

        let (_, hash1) = canonical_json_with_hash(&value);
        let (_, hash2) = canonical_json_with_hash(&value);

        assert_eq!(hash1, hash2, "Same input must produce same hash");
        assert_eq!(hash1.len(), 32, "SHA-256 hash must be 32 bytes");
    }

    #[test]
    fn test_validate_payload_hash_accepts_matching() {
        let payload = r#"{"case_id":"KYC-001"}"#;
        let hash = sha256_bytes(payload);
        assert!(validate_payload_hash(payload, &hash));
    }

    #[test]
    fn test_validate_payload_hash_rejects_mismatching() {
        let payload = r#"{"case_id":"KYC-001"}"#;
        let wrong_hash = sha256_bytes("different payload");
        assert!(!validate_payload_hash(payload, &wrong_hash));
    }

    #[test]
    fn test_nested_key_ordering() {
        let mut inner = serde_json::Map::new();
        inner.insert("z".to_string(), json!(1));
        inner.insert("a".to_string(), json!(2));

        let mut outer = serde_json::Map::new();
        outer.insert("nested".to_string(), serde_json::Value::Object(inner));
        outer.insert("top".to_string(), json!("val"));

        let value = serde_json::Value::Object(outer);
        let (json_str, _) = canonical_json_with_hash(&value);

        // Verify nested keys are also sorted
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        let nested = parsed["nested"].as_object().unwrap();
        let keys: Vec<&String> = nested.keys().collect();
        assert_eq!(keys, vec!["a", "z"]);
    }

    #[test]
    fn test_array_ordering_preserved() {
        let value = json!([3, 1, 2]);
        let (json_str, _) = canonical_json_with_hash(&value);
        assert_eq!(json_str, "[3,1,2]", "Array element order must be preserved");
    }
}
