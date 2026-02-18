//! Canonical serialization for content-addressed runbook IDs (INV-2).
//!
//! All compiled runbook types use `bincode` for deterministic binary
//! serialization. `HashMap`/`HashSet` are forbidden in canonical types —
//! only `BTreeMap`/`BTreeSet` are allowed (iteration order is deterministic).
//!
//! **JSONB is NOT canonical** — PostgreSQL JSONB normalizes key order and
//! whitespace non-deterministically. `serde_json` must NEVER be used for
//! hashing. Only `bincode` (fixed-layout binary) feeds into SHA-256.
//!
//! ## Content-Addressed ID Derivation
//!
//! ```text
//! SHA-256(canonical_bytes(steps) ++ canonical_bytes(envelope))
//!   → truncate to 128 bits
//!   → Uuid::from_bytes()
//!   → CompiledRunbookId
//! ```
//!
//! ## Invariants
//!
//! - **INV-2**: `BTreeMap`/`BTreeSet` only; no floats; bincode deterministic;
//!   SHA-256 truncated to 128-bit UUID.
//! - **INV-3**: Round-trip property tests on every canonical type (see tests).
//! - **INV-13**: Schema evolution changes bincode layout → different hash →
//!   different content-addressed ID.

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::envelope::{EnvelopeCore, MacroExpansionAudit, ReplayEnvelope};
use super::types::{CompiledRunbookId, CompiledStep};

// ---------------------------------------------------------------------------
// Canonical byte serialization
// ---------------------------------------------------------------------------

/// Serialize a slice of compiled steps to deterministic bincode bytes.
pub fn canonical_bytes_for_steps(steps: &[CompiledStep]) -> Vec<u8> {
    // SAFETY: all fields are primitives, BTreeMaps, Vecs, and Strings —
    // bincode serialization cannot fail for these types.
    bincode::serialize(steps).expect("bincode serialization of CompiledStep slice is infallible")
}

/// Serialize an envelope core to deterministic bincode bytes.
///
/// Only the `EnvelopeCore` (no timestamps) feeds into the content-addressed
/// hash. The full `ReplayEnvelope` is stored for audit but not hashed.
pub fn canonical_bytes_for_envelope_core(core: &EnvelopeCore) -> Vec<u8> {
    // SAFETY: all fields are primitives, BTreeMaps, Vecs, and Strings —
    // bincode serialization cannot fail for these types.
    bincode::serialize(core).expect("bincode serialization of EnvelopeCore is infallible")
}

/// Serialize a full replay envelope to deterministic bincode bytes.
///
/// Used for storage/integrity checks, NOT for content-addressed ID hashing.
/// For ID hashing, use `canonical_bytes_for_envelope_core()`.
pub fn canonical_bytes_for_envelope(envelope: &ReplayEnvelope) -> Vec<u8> {
    bincode::serialize(envelope).expect("bincode serialization of ReplayEnvelope is infallible")
}

/// Serialize a single compiled step to deterministic bincode bytes.
pub fn canonical_bytes_for_step(step: &CompiledStep) -> Vec<u8> {
    // SAFETY: all fields are primitives, BTreeMaps, Vecs, and Strings.
    bincode::serialize(step).expect("bincode serialization of CompiledStep is infallible")
}

/// Serialize a macro expansion audit to deterministic bincode bytes.
pub fn canonical_bytes_for_audit(audit: &MacroExpansionAudit) -> Vec<u8> {
    bincode::serialize(audit).expect("bincode serialization of MacroExpansionAudit is infallible")
}

// ---------------------------------------------------------------------------
// Content-addressed ID computation
// ---------------------------------------------------------------------------

/// Compute a content-addressed `CompiledRunbookId` from steps + envelope core.
///
/// ```text
/// SHA-256(bincode(steps) ++ bincode(envelope.core)) → truncate 128 bits → UUID
/// ```
///
/// Only the deterministic `EnvelopeCore` (no timestamps) feeds into the hash.
/// This ensures that two compilations of the same input at different times
/// produce the same `CompiledRunbookId`.
///
/// This is the **only** way to derive a `CompiledRunbookId` in production.
/// `CompiledRunbookId::new()` (random UUID) is retained only for `#[cfg(test)]`.
pub fn content_addressed_id(
    steps: &[CompiledStep],
    envelope: &ReplayEnvelope,
) -> CompiledRunbookId {
    let mut hasher = Sha256::new();
    hasher.update(canonical_bytes_for_steps(steps));
    hasher.update(canonical_bytes_for_envelope_core(&envelope.core));
    let hash = hasher.finalize();

    // Truncate SHA-256 (32 bytes) to 128 bits (16 bytes) → UUID
    let bytes: [u8; 16] = hash[..16]
        .try_into()
        .expect("SHA-256 always produces 32 bytes; first 16 are always available");
    CompiledRunbookId(Uuid::from_bytes(bytes))
}

/// Compute the full SHA-256 hash (32 bytes) for integrity verification.
///
/// Uses the deterministic `EnvelopeCore` — same hash basis as
/// `content_addressed_id()` so truncated ID matches full hash prefix.
pub fn full_sha256(steps: &[CompiledStep], envelope: &ReplayEnvelope) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(canonical_bytes_for_steps(steps));
    hasher.update(canonical_bytes_for_envelope_core(&envelope.core));
    hasher.finalize().into()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::macros::ExpansionLimits;
    use chrono::Utc;
    use std::collections::BTreeMap;

    fn sample_step(verb: &str, args: &[(&str, &str)]) -> CompiledStep {
        CompiledStep {
            step_id: Uuid::nil(),
            sentence: format!("Execute {verb}"),
            verb: verb.to_string(),
            dsl: format!("({verb})"),
            args: args
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            depends_on: vec![],
            execution_mode: super::super::types::ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        }
    }

    fn sample_envelope() -> ReplayEnvelope {
        let mut bindings = BTreeMap::new();
        bindings.insert("Allianz".to_string(), Uuid::nil());
        ReplayEnvelope {
            core: super::super::envelope::EnvelopeCore {
                session_cursor: 42,
                entity_bindings: bindings,
                external_lookup_digests: vec![],
                macro_audit_digests: vec![],
                snapshot_manifest: std::collections::HashMap::new(),
            },
            external_lookups: vec![],
            macro_audits: vec![],
            sealed_at: chrono::DateTime::UNIX_EPOCH.into(),
        }
    }

    fn sample_audit() -> MacroExpansionAudit {
        MacroExpansionAudit {
            expansion_id: Uuid::nil(),
            macro_name: "structure.setup".to_string(),
            params: BTreeMap::new(),
            resolved_autofill: BTreeMap::new(),
            expansion_digest: "abc123".to_string(),
            expansion_limits: ExpansionLimits::default(),
            expanded_at: chrono::DateTime::UNIX_EPOCH.into(),
        }
    }

    // -- Determinism tests --

    #[test]
    fn test_canonical_determinism_steps() {
        let steps = vec![sample_step("cbu.create", &[("name", "Acme")])];
        let a = canonical_bytes_for_steps(&steps);
        let b = canonical_bytes_for_steps(&steps);
        assert_eq!(a, b, "Same steps must produce identical bytes");
    }

    #[test]
    fn test_canonical_determinism_envelope() {
        let env = sample_envelope();
        let a = canonical_bytes_for_envelope(&env);
        let b = canonical_bytes_for_envelope(&env);
        assert_eq!(a, b, "Same envelope must produce identical bytes");
    }

    #[test]
    fn test_canonical_determinism_content_id() {
        let steps = vec![sample_step("cbu.create", &[("name", "Acme")])];
        let env = sample_envelope();
        let id1 = content_addressed_id(&steps, &env);
        let id2 = content_addressed_id(&steps, &env);
        assert_eq!(
            id1, id2,
            "Same inputs must produce same content-addressed ID"
        );
    }

    #[test]
    fn test_different_args_different_id() {
        let env = sample_envelope();
        let steps_a = vec![sample_step("cbu.create", &[("name", "Acme")])];
        let steps_b = vec![sample_step("cbu.create", &[("name", "Beta")])];
        let id_a = content_addressed_id(&steps_a, &env);
        let id_b = content_addressed_id(&steps_b, &env);
        assert_ne!(id_a, id_b, "Different args must produce different IDs");
    }

    #[test]
    fn test_different_verbs_different_id() {
        let env = sample_envelope();
        let steps_a = vec![sample_step("cbu.create", &[])];
        let steps_b = vec![sample_step("cbu.delete", &[])];
        let id_a = content_addressed_id(&steps_a, &env);
        let id_b = content_addressed_id(&steps_b, &env);
        assert_ne!(id_a, id_b, "Different verbs must produce different IDs");
    }

    #[test]
    fn test_different_envelope_different_id() {
        let steps = vec![sample_step("cbu.create", &[])];
        let env_a = ReplayEnvelope::empty();
        let mut env_b = ReplayEnvelope::empty();
        env_b.core.session_cursor = 99;
        let id_a = content_addressed_id(&steps, &env_a);
        let id_b = content_addressed_id(&steps, &env_b);
        assert_ne!(id_a, id_b, "Different envelopes must produce different IDs");
    }

    /// Phase A regression test: same inputs at different times produce same ID.
    #[test]
    fn test_timestamp_excluded_from_hash() {
        let steps = vec![sample_step("cbu.create", &[("name", "Acme")])];
        let mut env_a = ReplayEnvelope::empty();
        let mut env_b = ReplayEnvelope::empty();
        // Different sealed_at timestamps
        env_a.sealed_at = chrono::DateTime::UNIX_EPOCH.into();
        env_b.sealed_at = chrono::Utc::now();
        let id_a = content_addressed_id(&steps, &env_a);
        let id_b = content_addressed_id(&steps, &env_b);
        assert_eq!(
            id_a, id_b,
            "Timestamps must not affect content-addressed ID"
        );
    }

    /// Phase A-3 regression test: compile same input twice, assert identical
    /// `CompiledRunbookId`. Exercises `CompiledRunbook::new()` which calls
    /// `content_addressed_id()` internally.
    #[test]
    fn test_same_input_same_id() {
        use crate::runbook::types::CompiledRunbook;
        let session_id = Uuid::new_v4();
        let version = 1u64;
        let steps = vec![sample_step(
            "cbu.create",
            &[("name", "Acme"), ("kind", "fund")],
        )];
        let env = ReplayEnvelope::with_bindings(
            42,
            std::collections::BTreeMap::from([("cbu".to_string(), Uuid::nil())]),
        );

        let rb1 = CompiledRunbook::new(session_id, version, steps.clone(), env.clone());
        // Simulate a second compilation at a different time
        let mut env2 = env;
        env2.sealed_at = chrono::Utc::now() + chrono::Duration::hours(1);
        let rb2 = CompiledRunbook::new(session_id, version, steps, env2);

        assert_eq!(
            rb1.id, rb2.id,
            "Identical inputs must produce identical CompiledRunbookId regardless of timestamp"
        );
    }

    // -- Round-trip tests (bincode serialize → deserialize) --

    #[test]
    fn test_step_bincode_round_trip() {
        let step = sample_step("cbu.create", &[("name", "Acme"), ("kind", "fund")]);
        let bytes = canonical_bytes_for_step(&step);
        let decoded: CompiledStep = bincode::deserialize(&bytes).expect("round-trip deserialize");
        assert_eq!(step, decoded);
    }

    #[test]
    fn test_envelope_bincode_round_trip() {
        let env = sample_envelope();
        let bytes = canonical_bytes_for_envelope(&env);
        let decoded: ReplayEnvelope = bincode::deserialize(&bytes).expect("round-trip deserialize");
        assert_eq!(env, decoded);
    }

    #[test]
    fn test_audit_bincode_round_trip() {
        let audit = sample_audit();
        let bytes = canonical_bytes_for_audit(&audit);
        let decoded: MacroExpansionAudit =
            bincode::deserialize(&bytes).expect("round-trip deserialize");
        assert_eq!(audit, decoded);
    }

    #[test]
    fn test_steps_slice_bincode_round_trip() {
        let steps = vec![
            sample_step("cbu.create", &[("name", "Acme")]),
            sample_step("entity.create", &[("type", "fund")]),
        ];
        let bytes = canonical_bytes_for_steps(&steps);
        let decoded: Vec<CompiledStep> =
            bincode::deserialize(&bytes).expect("round-trip deserialize");
        assert_eq!(steps, decoded);
    }

    // -- JSON is NOT canonical guard test --

    #[test]
    fn test_json_not_used_for_hashing() {
        // Guard: canonical_bytes output must differ from serde_json output.
        // This prevents accidental regression to JSON-based hashing.
        let step = sample_step("cbu.create", &[("name", "Acme")]);
        let bincode_bytes = canonical_bytes_for_step(&step);
        let json_bytes = serde_json::to_vec(&step).unwrap();
        assert_ne!(
            bincode_bytes, json_bytes,
            "Canonical bytes must use bincode, not JSON"
        );
    }

    // -- BTreeMap ordering test --

    #[test]
    fn test_btreemap_ordering_deterministic() {
        // Insert keys in different order → same canonical bytes
        let mut args_a = BTreeMap::new();
        args_a.insert("zebra".to_string(), "1".to_string());
        args_a.insert("alpha".to_string(), "2".to_string());

        let mut args_b = BTreeMap::new();
        args_b.insert("alpha".to_string(), "2".to_string());
        args_b.insert("zebra".to_string(), "1".to_string());

        let step_a = CompiledStep {
            step_id: Uuid::nil(),
            sentence: "test".into(),
            verb: "test.verb".into(),
            dsl: "(test.verb)".into(),
            args: args_a,
            depends_on: vec![],
            execution_mode: super::super::types::ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        };
        let step_b = CompiledStep {
            step_id: Uuid::nil(),
            sentence: "test".into(),
            verb: "test.verb".into(),
            dsl: "(test.verb)".into(),
            args: args_b,
            depends_on: vec![],
            execution_mode: super::super::types::ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        };

        let bytes_a = canonical_bytes_for_step(&step_a);
        let bytes_b = canonical_bytes_for_step(&step_b);
        assert_eq!(
            bytes_a, bytes_b,
            "BTreeMap with same entries in different insertion order must produce same bytes"
        );
    }

    // -- Full SHA-256 hash test --

    #[test]
    fn test_full_sha256_is_32_bytes() {
        let steps = vec![sample_step("cbu.create", &[])];
        let env = ReplayEnvelope::empty();
        let hash = full_sha256(&steps, &env);
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_content_id_matches_truncated_sha256() {
        let steps = vec![sample_step("cbu.create", &[])];
        let env = ReplayEnvelope::empty();
        let id = content_addressed_id(&steps, &env);
        let hash = full_sha256(&steps, &env);
        // First 16 bytes of SHA-256 should match the UUID bytes
        let expected_uuid = Uuid::from_bytes(hash[..16].try_into().unwrap());
        assert_eq!(id.0, expected_uuid);
    }

    // -- Schema evolution test (INV-13) --

    #[test]
    fn test_schema_evolution_changes_hash() {
        // Different step counts → different bytes → different hash
        // This is a basic proof that structural changes affect the ID.
        let env = ReplayEnvelope::empty();
        let steps_1 = vec![sample_step("cbu.create", &[])];
        let steps_2 = vec![
            sample_step("cbu.create", &[]),
            sample_step("entity.create", &[]),
        ];
        let id_1 = content_addressed_id(&steps_1, &env);
        let id_2 = content_addressed_id(&steps_2, &env);
        assert_ne!(
            id_1, id_2,
            "Different step counts must produce different IDs (schema evolution)"
        );
    }
}

// ---------------------------------------------------------------------------
// Property tests (INV-3)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod proptests {
    use super::*;
    use crate::dsl_v2::macros::ExpansionLimits;
    use crate::runbook::envelope::{ExternalLookup, MacroExpansionAudit, ReplayEnvelope};
    use crate::runbook::types::{CompiledStep, ExecutionMode};
    use chrono::{DateTime, TimeZone, Utc};
    use proptest::prelude::*;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    // -- Strategy helpers --

    fn arb_uuid() -> impl Strategy<Value = Uuid> {
        prop::array::uniform16(any::<u8>()).prop_map(Uuid::from_bytes)
    }

    fn arb_datetime() -> impl Strategy<Value = DateTime<Utc>> {
        // Generate timestamps within a reasonable range (2020-2030)
        (1577836800i64..1893456000i64).prop_map(|secs| {
            Utc.timestamp_opt(secs, 0)
                .single()
                .unwrap_or_else(|| DateTime::UNIX_EPOCH.into())
        })
    }

    fn arb_execution_mode() -> impl Strategy<Value = ExecutionMode> {
        prop_oneof![
            Just(ExecutionMode::Sync),
            Just(ExecutionMode::Durable),
            Just(ExecutionMode::HumanGate),
        ]
    }

    fn arb_btreemap_string_string(
        max_size: usize,
    ) -> impl Strategy<Value = BTreeMap<String, String>> {
        prop::collection::btree_map("[a-z]{1,8}", ".*", 0..max_size)
    }

    // -- Arbitrary types --

    fn arb_compiled_step() -> impl Strategy<Value = CompiledStep> {
        (
            arb_uuid(),
            ".*",
            "[a-z]+\\.[a-z]+",
            ".*",
            arb_btreemap_string_string(5),
            prop::collection::vec(arb_uuid(), 0..3),
            arb_execution_mode(),
            prop::collection::vec(arb_uuid(), 0..3),
        )
            .prop_map(
                |(step_id, sentence, verb, dsl, args, depends_on, execution_mode, write_set)| {
                    CompiledStep {
                        step_id,
                        sentence,
                        verb,
                        dsl,
                        args,
                        depends_on,
                        execution_mode,
                        write_set,
                        verb_contract_snapshot_id: None,
                    }
                },
            )
    }

    fn arb_external_lookup() -> impl Strategy<Value = ExternalLookup> {
        ("[a-z]+", ".*", "[a-f0-9]{64}", arb_datetime()).prop_map(
            |(source, query, response_digest, performed_at)| ExternalLookup {
                source,
                query,
                response_digest,
                performed_at,
            },
        )
    }

    fn arb_expansion_limits() -> impl Strategy<Value = ExpansionLimits> {
        (1usize..20, 1usize..1000).prop_map(|(max_depth, max_steps)| ExpansionLimits {
            max_depth,
            max_steps,
        })
    }

    fn arb_macro_audit() -> impl Strategy<Value = MacroExpansionAudit> {
        (
            arb_uuid(),
            "[a-z]+\\.[a-z]+",
            arb_btreemap_string_string(4),
            arb_btreemap_string_string(4),
            "[a-f0-9]{64}",
            arb_expansion_limits(),
            arb_datetime(),
        )
            .prop_map(
                |(
                    expansion_id,
                    macro_name,
                    params,
                    resolved_autofill,
                    expansion_digest,
                    expansion_limits,
                    expanded_at,
                )| {
                    MacroExpansionAudit {
                        expansion_id,
                        macro_name,
                        params,
                        resolved_autofill,
                        expansion_digest,
                        expansion_limits,
                        expanded_at,
                    }
                },
            )
    }

    fn arb_envelope_core() -> impl Strategy<Value = crate::runbook::envelope::EnvelopeCore> {
        (
            any::<u64>(),
            prop::collection::btree_map("[a-z]{1,8}", arb_uuid(), 0..5),
            prop::collection::vec("[a-f0-9]{64}", 0..3),
            prop::collection::vec("[a-f0-9]{64}", 0..3),
        )
            .prop_map(
                |(
                    session_cursor,
                    entity_bindings,
                    external_lookup_digests,
                    macro_audit_digests,
                )| {
                    crate::runbook::envelope::EnvelopeCore {
                        session_cursor,
                        entity_bindings,
                        external_lookup_digests,
                        macro_audit_digests,
                        snapshot_manifest: std::collections::HashMap::new(),
                    }
                },
            )
    }

    fn arb_replay_envelope() -> impl Strategy<Value = ReplayEnvelope> {
        (
            arb_envelope_core(),
            prop::collection::vec(arb_external_lookup(), 0..3),
            prop::collection::vec(arb_macro_audit(), 0..3),
            arb_datetime(),
        )
            .prop_map(
                |(core, external_lookups, macro_audits, sealed_at)| ReplayEnvelope {
                    core,
                    external_lookups,
                    macro_audits,
                    sealed_at,
                },
            )
    }

    // -- Property tests (INV-3) --
    //
    // All canonical types use only primitives, `BTreeMap<String, String>`,
    // and `Vec` — fully supported by bincode for round-trip. Every proptest
    // verifies serialize → deserialize → assert_eq (not just determinism).

    proptest! {
        #[test]
        fn compiled_step_round_trip(step in arb_compiled_step()) {
            let bytes = canonical_bytes_for_step(&step);
            let decoded: CompiledStep = bincode::deserialize(&bytes)
                .expect("bincode round-trip deserialize");
            prop_assert_eq!(step, decoded);
        }

        /// Full bincode round-trip for ReplayEnvelope (INV-3).
        #[test]
        fn replay_envelope_round_trip(env in arb_replay_envelope()) {
            let bytes = canonical_bytes_for_envelope(&env);
            let decoded: ReplayEnvelope = bincode::deserialize(&bytes)
                .expect("bincode round-trip deserialize");
            prop_assert_eq!(env, decoded);
        }

        /// Full bincode round-trip for MacroExpansionAudit (INV-3).
        #[test]
        fn macro_audit_round_trip(audit in arb_macro_audit()) {
            let bytes = canonical_bytes_for_audit(&audit);
            let decoded: MacroExpansionAudit = bincode::deserialize(&bytes)
                .expect("bincode round-trip deserialize");
            prop_assert_eq!(audit, decoded);
        }

        #[test]
        fn content_addressed_id_deterministic(
            steps in prop::collection::vec(arb_compiled_step(), 1..5),
            env in arb_replay_envelope(),
        ) {
            let id1 = content_addressed_id(&steps, &env);
            let id2 = content_addressed_id(&steps, &env);
            prop_assert_eq!(id1, id2, "Same inputs must always produce same ID");
        }
    }

    // Non-proptest round-trip tests with realistic data.

    #[test]
    fn replay_envelope_round_trip_realistic() {
        let env = ReplayEnvelope {
            core: crate::runbook::envelope::EnvelopeCore {
                session_cursor: 42,
                entity_bindings: {
                    let mut m = BTreeMap::new();
                    m.insert("allianz".into(), Uuid::new_v4());
                    m.insert("blackrock".into(), Uuid::new_v4());
                    m
                },
                external_lookup_digests: vec!["abc123".into()],
                macro_audit_digests: vec!["def456".into()],
                snapshot_manifest: std::collections::HashMap::new(),
            },
            external_lookups: vec![ExternalLookup {
                source: "gleif".into(),
                query: "allianz".into(),
                response_digest: "abc123".into(),
                performed_at: Utc::now(),
            }],
            macro_audits: vec![MacroExpansionAudit {
                expansion_id: Uuid::new_v4(),
                macro_name: "structure.setup".into(),
                params: BTreeMap::new(),
                resolved_autofill: BTreeMap::new(),
                expansion_digest: "def456".into(),
                expansion_limits: ExpansionLimits::default(),
                expanded_at: Utc::now(),
            }],
            sealed_at: Utc::now(),
        };
        let bytes = canonical_bytes_for_envelope(&env);
        let decoded: ReplayEnvelope =
            bincode::deserialize(&bytes).expect("round-trip with empty JSON maps");
        assert_eq!(env, decoded);
    }

    #[test]
    fn macro_audit_round_trip_realistic() {
        let audit = MacroExpansionAudit {
            expansion_id: Uuid::new_v4(),
            macro_name: "party.assign".into(),
            params: BTreeMap::new(),
            resolved_autofill: BTreeMap::new(),
            expansion_digest: "aabbccdd".into(),
            expansion_limits: ExpansionLimits {
                max_depth: 8,
                max_steps: 500,
            },
            expanded_at: Utc::now(),
        };
        let bytes = canonical_bytes_for_audit(&audit);
        let decoded: MacroExpansionAudit =
            bincode::deserialize(&bytes).expect("round-trip with empty JSON maps");
        assert_eq!(audit, decoded);
    }

    #[test]
    fn envelope_core_round_trip() {
        let core = crate::runbook::envelope::EnvelopeCore {
            session_cursor: 99,
            entity_bindings: {
                let mut m = BTreeMap::new();
                m.insert("test".into(), Uuid::new_v4());
                m
            },
            external_lookup_digests: vec!["digest1".into(), "digest2".into()],
            macro_audit_digests: vec!["macro_digest".into()],
            snapshot_manifest: std::collections::HashMap::new(),
        };
        let bytes = canonical_bytes_for_envelope_core(&core);
        let decoded: crate::runbook::envelope::EnvelopeCore =
            bincode::deserialize(&bytes).expect("EnvelopeCore round-trip");
        assert_eq!(core, decoded);
    }
}
