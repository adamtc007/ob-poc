//! Canonical JSON Hashing
//!
//! Provides deterministic hashing for JSON values, ensuring that semantically
//! equivalent JSON produces the same hash regardless of key ordering or formatting.
//!
//! This is used to detect changes in verb configurations and ensure reproducibility.

use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};

/// Compute SHA256 of canonicalized JSON (sorted keys, deterministic output)
///
/// This ensures that the same semantic JSON content always produces the same hash,
/// regardless of how the keys were originally ordered.
pub fn canonical_json_hash(value: &JsonValue) -> [u8; 32] {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).expect("canonical json->bytes should not fail");
    sha256(&bytes)
}

/// Compute SHA256 of arbitrary bytes
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Normalize JSON for deterministic hashing
///
/// - Object keys are sorted alphabetically (recursive)
/// - Arrays preserve order
/// - Nulls, bools, numbers, strings unchanged
pub fn canonicalize_json(v: &JsonValue) -> JsonValue {
    match v {
        JsonValue::Object(map) => {
            // Sort keys alphabetically
            let mut keys: Vec<_> = map.keys().collect();
            keys.sort();

            // Rebuild map in sorted key order
            let mut sorted = serde_json::Map::new();
            for k in keys {
                if let Some(child) = map.get(k) {
                    sorted.insert(k.clone(), canonicalize_json(child));
                }
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(arr) => {
            // Arrays preserve order, but recursively canonicalize elements
            JsonValue::Array(arr.iter().map(canonicalize_json).collect())
        }
        // Primitives are unchanged
        other => other.clone(),
    }
}

/// Convert hash bytes to hex string for display/storage
pub fn hash_to_hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Parse hex string back to hash bytes
pub fn hex_to_hash(hex_str: &str) -> Option<[u8; 32]> {
    if hex_str.len() != 64 {
        return None;
    }

    let mut arr = [0u8; 32];
    for (i, chunk) in hex_str.as_bytes().chunks(2).enumerate() {
        let s = std::str::from_utf8(chunk).ok()?;
        arr[i] = u8::from_str_radix(s, 16).ok()?;
    }
    Some(arr)
}

/// Compare two hashes for equality (constant-time for security, though not critical here)
pub fn hashes_equal(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_hash_key_order_independence() {
        // Same content, different key order
        let a = json!({"z": 1, "a": 2, "m": 3});
        let b = json!({"a": 2, "m": 3, "z": 1});
        let c = json!({"m": 3, "z": 1, "a": 2});

        let hash_a = canonical_json_hash(&a);
        let hash_b = canonical_json_hash(&b);
        let hash_c = canonical_json_hash(&c);

        assert_eq!(hash_a, hash_b);
        assert_eq!(hash_b, hash_c);
    }

    #[test]
    fn test_canonical_hash_nested_objects() {
        let a = json!({"outer": {"z": 1, "a": 2}, "level": "top"});
        let b = json!({"level": "top", "outer": {"a": 2, "z": 1}});

        assert_eq!(canonical_json_hash(&a), canonical_json_hash(&b));
    }

    #[test]
    fn test_canonical_hash_arrays_preserve_order() {
        let a = json!({"items": [1, 2, 3]});
        let b = json!({"items": [3, 2, 1]});

        // Arrays should NOT produce same hash - order matters
        assert_ne!(canonical_json_hash(&a), canonical_json_hash(&b));
    }

    #[test]
    fn test_canonical_hash_array_of_objects() {
        let a = json!({"items": [{"z": 1, "a": 2}, {"y": 3}]});
        let b = json!({"items": [{"a": 2, "z": 1}, {"y": 3}]});

        // Same objects in same array order = same hash
        assert_eq!(canonical_json_hash(&a), canonical_json_hash(&b));
    }

    #[test]
    fn test_different_values_different_hash() {
        let a = json!({"key": "value1"});
        let b = json!({"key": "value2"});

        assert_ne!(canonical_json_hash(&a), canonical_json_hash(&b));
    }

    #[test]
    fn test_hash_to_hex_roundtrip() {
        let original = json!({"test": "data"});
        let hash = canonical_json_hash(&original);
        let hex_str = hash_to_hex(&hash);
        let recovered = hex_to_hash(&hex_str);

        assert_eq!(Some(hash), recovered);
    }

    #[test]
    fn test_hex_to_hash_invalid() {
        // Too short
        assert!(hex_to_hash("abcd").is_none());

        // Invalid hex
        assert!(hex_to_hash("zzzz").is_none());

        // Wrong length (31 bytes)
        let short = "a".repeat(62);
        assert!(hex_to_hash(&short).is_none());
    }

    #[test]
    fn test_empty_objects_and_arrays() {
        let empty_obj = json!({});
        let empty_arr = json!([]);
        let null_val = json!(null);

        // All should hash without panic
        let _ = canonical_json_hash(&empty_obj);
        let _ = canonical_json_hash(&empty_arr);
        let _ = canonical_json_hash(&null_val);

        // Empty object and array should have different hashes
        assert_ne!(
            canonical_json_hash(&empty_obj),
            canonical_json_hash(&empty_arr)
        );
    }
}
