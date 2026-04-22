//! Phase C.2-main token-to-DagNodeId resolver (2026-04-22).
//!
//! The 72+ `emit_pending_state_advance` call sites across
//! `sem_os_postgres::ops::*` + `rust/src/domain_ops::*` produce
//! taxonomy-string `to_node` tokens like `"cbu:onboarded"` or
//! `"entity:ghost"`. The typed [`ob_poc_types::PendingStateAdvance`]
//! shape expects `to_node: DagNodeId(Uuid)`. Something has to bridge.
//!
//! This module is that bridge. It converts a token into a stable
//! [`DagNodeId`] via a deterministic BLAKE3 hash so the same
//! taxonomy string always maps to the same UUID — across runs,
//! across processes, across databases. No registry lookup, no I/O,
//! no persistence. Just a pure function.
//!
//! ## Design notes
//!
//! - **Deterministic:** `resolve("cbu:onboarded")` returns the same
//!   `DagNodeId` on every call, forever.
//! - **Normalisation:** the input is trimmed + lowercased before
//!   hashing so minor casing drift (`CBU:ONBOARDED` vs
//!   `cbu:onboarded`) collapses to the same id.
//! - **UUIDv8:** the hash is formatted as a UUIDv8 (RFC 9562 custom
//!   version). The version + variant bits are set; the remaining 122
//!   bits come from the BLAKE3 hash. Collision probability across all
//!   state tokens in the system is astronomically below any concern.
//! - **No YAML dependency:** the resolver has no external state. This
//!   is a deliberate choice — upgrading specific states to explicit
//!   UUIDs (if renaming-stability becomes a requirement) is a later
//!   refinement that can coexist with the hash default.
//!
//! ## When renaming is required
//!
//! If a state token needs to change its display name without breaking
//! the resolved UUID of in-flight or persisted references, promote it
//! to an explicit entry in a (future) YAML registry that overrides the
//! hash. Until that registry exists, renames produce a fresh UUID —
//! which is safe for now because the C.2-main apply path only
//! consumes newly-emitted advances; nothing is persisted yet.

use blake3;
use uuid::Uuid;

use crate::DagNodeId;

/// Resolve a taxonomy token into a [`DagNodeId`] via deterministic
/// BLAKE3 hash. Pure function; no I/O.
///
/// The token format is `<namespace>:<state>` (e.g. `"cbu:onboarded"`,
/// `"entity:ghost"`, `"capital:transferred_out"`). Hyphens and
/// underscores are preserved as typed; only whitespace and case are
/// normalised.
pub fn resolve_state_token(token: &str) -> DagNodeId {
    let normalised = token.trim().to_lowercase();
    let hash = blake3::hash(normalised.as_bytes());
    let bytes = hash.as_bytes();

    let mut uuid_bytes: [u8; 16] = bytes[..16]
        .try_into()
        .expect("BLAKE3 hash is at least 32 bytes");

    // RFC 9562 UUIDv8: version bits 0b1000 in the top 4 bits of byte 6.
    uuid_bytes[6] = (uuid_bytes[6] & 0x0F) | 0x80;
    // Variant bits 0b10 in the top 2 bits of byte 8.
    uuid_bytes[8] = (uuid_bytes[8] & 0x3F) | 0x80;

    DagNodeId(Uuid::from_bytes(uuid_bytes))
}

/// Resolve every `to_node` token inside a raw (pre-resolution) emit
/// payload and return the payload with the tokens replaced by UUID
/// strings. Non-destructive: takes a `&Value`, returns a new `Value`.
///
/// Used by the dispatcher's shadow-log path. When the real Phase
/// C.2-main apply-in-txn lands, it'll call the same resolver to
/// construct a typed [`PendingStateAdvance`] before applying it via
/// SemOS.
pub fn resolve_pending_state_advance(raw: &serde_json::Value) -> serde_json::Value {
    let mut resolved = raw.clone();

    if let Some(obj) = resolved.as_object_mut() {
        if let Some(serde_json::Value::Array(transitions)) = obj.get_mut("state_transitions") {
            for t in transitions.iter_mut() {
                if let Some(t_obj) = t.as_object_mut() {
                    if let Some(serde_json::Value::String(to_node)) = t_obj.get("to_node").cloned()
                    {
                        let resolved_id = resolve_state_token(&to_node);
                        t_obj.insert(
                            "to_node_resolved".to_string(),
                            serde_json::Value::String(resolved_id.0.to_string()),
                        );
                    }
                }
            }
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_is_deterministic() {
        let a = resolve_state_token("cbu:onboarded");
        let b = resolve_state_token("cbu:onboarded");
        assert_eq!(a, b, "same token must produce same DagNodeId");
    }

    #[test]
    fn resolver_is_case_insensitive() {
        let lower = resolve_state_token("cbu:onboarded");
        let upper = resolve_state_token("CBU:ONBOARDED");
        let mixed = resolve_state_token("  Cbu:OnBoarded  ");
        assert_eq!(lower, upper);
        assert_eq!(lower, mixed, "leading/trailing whitespace is stripped");
    }

    #[test]
    fn different_tokens_produce_different_ids() {
        let a = resolve_state_token("cbu:onboarded");
        let b = resolve_state_token("cbu:active");
        assert_ne!(a, b);
    }

    #[test]
    fn hyphens_and_underscores_distinct() {
        // kebab vs snake yields different UUIDs — renaming a state
        // across that boundary WILL change its id. See the module
        // doc-comment for the "promote to explicit UUID" escape hatch.
        let kebab = resolve_state_token("cbu-role:ownership-assigned");
        let snake = resolve_state_token("cbu_role:ownership_assigned");
        assert_ne!(kebab, snake);
    }

    #[test]
    fn uuid_version_and_variant_bits_are_correct() {
        let id = resolve_state_token("test:state");
        let bytes = id.0.as_bytes();

        // Version bits (top 4 of byte 6) should be 0b1000 = 0x80.
        assert_eq!(bytes[6] & 0xF0, 0x80, "UUIDv8 version marker");
        // Variant bits (top 2 of byte 8) should be 0b10 = 0x80.
        assert_eq!(bytes[8] & 0xC0, 0x80, "RFC 9562 variant marker");
    }

    #[test]
    fn resolve_raw_advance_adds_to_node_resolved() {
        // The emit helper writes JSON like:
        // { state_transitions: [{ to_node: "cbu:onboarded", ... }], ... }
        let raw = serde_json::json!({
            "state_transitions": [
                { "entity_id": "00000000-0000-0000-0000-000000000001",
                  "from_node": null,
                  "to_node": "cbu:onboarded",
                  "reason": "test" }
            ],
            "constellation_marks": [],
            "writes_since_push_delta": 1,
            "catalogue_effects": [],
        });

        let resolved = resolve_pending_state_advance(&raw);
        let transitions = resolved["state_transitions"].as_array().unwrap();
        assert_eq!(transitions.len(), 1);

        let t0 = &transitions[0];
        // Original to_node preserved.
        assert_eq!(t0["to_node"], "cbu:onboarded");
        // New resolved UUID appended.
        let resolved_str = t0["to_node_resolved"].as_str().unwrap();
        let expected = resolve_state_token("cbu:onboarded").0.to_string();
        assert_eq!(resolved_str, expected);
    }

    #[test]
    fn resolve_raw_advance_missing_to_node_is_noop() {
        // If a transition lacks to_node entirely, resolver skips it
        // silently (doesn't inject the key).
        let raw = serde_json::json!({
            "state_transitions": [
                { "entity_id": "00000000-0000-0000-0000-000000000001" }
            ]
        });
        let resolved = resolve_pending_state_advance(&raw);
        assert!(resolved["state_transitions"][0].get("to_node_resolved").is_none());
    }
}
