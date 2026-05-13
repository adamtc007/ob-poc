//! JSON envelope for runbook source + state context — Phase 4.5
//! (D2=c).
//!
//! Locked decision D2=c (Sage ACP capability plan §6): the DSL
//! grammar stays stateless; per-prompt state context (the
//! `entity-type:uuid` references the editor binds in) lives in a
//! sibling JSON map. The envelope of `{source, state_context,
//! runbook_id, version}` becomes the **hashable audit artefact**.
//! Phase 5.5 (determinism harness) verifies byte-equality across
//! BYOK replay against this hash.
//!
//! ## Why not a grammar change?
//!
//! The plan rejected D2=a (frontmatter sexp baked into the
//! grammar) because of the ripple it would cause through every
//! existing pack, fixture, parser, tree-sitter grammar, and LSP.
//! D2=c gives audit-grade reproducibility (single hashable
//! artefact) without forcing the grammar to carry state, and lets
//! editors / agents send the envelope to the validator unchanged.
//!
//! ## Shape
//!
//! - `runbook_id` — stable per-runbook identifier (UUID, slug,
//!   etc.) carried across revisions.
//! - `version` — monotonic per-runbook counter. Bumped on every
//!   `didChange` cycle so audit can replay the exact revision.
//! - `source` — stateless DSL text (the body that `dsl_core::parser`
//!   accepts).
//! - `state_context` — free-form key/value map of state references
//!   the source needs at validate / execute time
//!   (`@cbu = "entity:cbu:abc-123"`, jurisdiction tags, …).
//!   Substrate-side validators read the same map.
//! - `envelope_hash` — SHA-256 of the canonical JSON serialisation
//!   (sorted keys, no whitespace). Computed by [`RunbookEnvelope::
//!   envelope_hash`]; clients store it alongside the envelope and
//!   re-verify on replay.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Replay-grade runbook envelope. The single hashable audit
/// artefact V&S §6.5 / D2=c specifies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunbookEnvelope {
    pub runbook_id: String,
    pub version: u32,
    pub source: String,
    /// `BTreeMap` so canonical serialisation is stable (sorted
    /// keys, deterministic byte layout for hashing).
    pub state_context: BTreeMap<String, Value>,
}

impl RunbookEnvelope {
    /// Construct a fresh envelope at `version = 1`.
    pub fn new(
        runbook_id: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            runbook_id: runbook_id.into(),
            version: 1,
            source: source.into(),
            state_context: BTreeMap::new(),
        }
    }

    /// Construct a fresh envelope with a populated state context.
    pub fn with_state_context(
        runbook_id: impl Into<String>,
        source: impl Into<String>,
        state_context: BTreeMap<String, Value>,
    ) -> Self {
        Self {
            runbook_id: runbook_id.into(),
            version: 1,
            source: source.into(),
            state_context,
        }
    }

    /// Bump `version` and replace `source`. Used by `didChange`.
    pub fn revise(&mut self, new_source: impl Into<String>) {
        self.source = new_source.into();
        self.version = self.version.saturating_add(1);
    }

    /// Apply an in-place mutation to `state_context` and bump
    /// `version`. The mutation closure must return `()` so the
    /// envelope retains ownership; on completion, the version
    /// always advances even if no actual change happened, so audit
    /// reads can correlate.
    pub fn apply_state_change<F>(&mut self, mutate: F)
    where
        F: FnOnce(&mut BTreeMap<String, Value>),
    {
        mutate(&mut self.state_context);
        self.version = self.version.saturating_add(1);
    }

    /// SHA-256 of the canonical (sorted-key, no-whitespace) JSON
    /// serialisation of the envelope. Stable across runs and
    /// `serde_json` versions because the `BTreeMap` already sorts.
    pub fn envelope_hash(&self) -> String {
        // Canonical JSON via `to_value` then `to_string` — both
        // honour `BTreeMap` ordering and produce no whitespace.
        let canonical = serde_json::to_string(&serde_json::to_value(self).expect("serializable"))
            .expect("re-serialise");
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let bytes = hasher.finalize();
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            let _ = write!(hex, "{byte:02x}");
        }
        hex
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_initialises_version_one() {
        let envelope = RunbookEnvelope::new("rb-1", "(cbu.create)");
        assert_eq!(envelope.runbook_id, "rb-1");
        assert_eq!(envelope.version, 1);
        assert_eq!(envelope.source, "(cbu.create)");
        assert!(envelope.state_context.is_empty());
    }

    #[test]
    fn revise_bumps_version_and_replaces_source() {
        let mut envelope = RunbookEnvelope::new("rb-1", "(cbu.create)");
        envelope.revise("(cbu.attach-product)");
        assert_eq!(envelope.version, 2);
        assert_eq!(envelope.source, "(cbu.attach-product)");
    }

    #[test]
    fn apply_state_change_bumps_version() {
        let mut envelope = RunbookEnvelope::new("rb-1", "(cbu.create)");
        envelope.apply_state_change(|ctx| {
            ctx.insert("cbu".to_string(), json!("entity:cbu:abc"));
        });
        assert_eq!(envelope.version, 2);
        assert_eq!(envelope.state_context["cbu"], json!("entity:cbu:abc"));
    }

    #[test]
    fn envelope_hash_is_stable_across_runs() {
        let envelope = RunbookEnvelope::new("rb-1", "(cbu.create)");
        let h1 = envelope.envelope_hash();
        let h2 = envelope.envelope_hash();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64, "SHA-256 hex string");
    }

    #[test]
    fn envelope_hash_changes_when_source_changes() {
        let envelope_a = RunbookEnvelope::new("rb-1", "(cbu.create)");
        let mut envelope_b = envelope_a.clone();
        envelope_b.revise("(cbu.attach-product)");
        assert_ne!(envelope_a.envelope_hash(), envelope_b.envelope_hash());
    }

    #[test]
    fn envelope_hash_changes_when_state_context_changes() {
        let envelope_a = RunbookEnvelope::new("rb-1", "(cbu.create)");
        let mut envelope_b = envelope_a.clone();
        envelope_b.apply_state_change(|ctx| {
            ctx.insert("cbu".to_string(), json!("entity:cbu:abc"));
        });
        assert_ne!(envelope_a.envelope_hash(), envelope_b.envelope_hash());
    }

    #[test]
    fn envelope_hash_is_stable_under_insertion_order() {
        // BTreeMap guarantees this — but we assert it so future
        // refactors don't silently regress.
        let mut envelope_a = RunbookEnvelope::new("rb-1", "(cbu.create)");
        envelope_a.apply_state_change(|ctx| {
            ctx.insert("alpha".to_string(), json!(1));
            ctx.insert("beta".to_string(), json!(2));
        });
        let mut envelope_b = RunbookEnvelope::new("rb-1", "(cbu.create)");
        envelope_b.apply_state_change(|ctx| {
            ctx.insert("beta".to_string(), json!(2));
            ctx.insert("alpha".to_string(), json!(1));
        });
        // Same version, same source, same context — hashes match.
        assert_eq!(envelope_a.envelope_hash(), envelope_b.envelope_hash());
    }

    #[test]
    fn envelope_json_round_trip() {
        let mut envelope = RunbookEnvelope::with_state_context(
            "rb-99",
            "(cbu.create)",
            BTreeMap::from([
                ("jurisdiction".to_string(), json!("LU")),
                ("vehicle".to_string(), json!("SICAV")),
            ]),
        );
        envelope.revise("(cbu.attach-product :cbu @cbu)");
        let s = serde_json::to_string(&envelope).unwrap();
        let parsed: RunbookEnvelope = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, envelope);
    }
}
