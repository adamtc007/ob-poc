//! Replay envelope — captures the determinism boundary for a compiled runbook.
//!
//! The envelope records everything that was non-deterministic at compile time
//! (entity bindings, external lookups, macro expansion audits) so that a
//! compiled runbook can be replayed deterministically.
//!
//! ## Invariant (INV-2)
//!
//! Given the same `ReplayEnvelope`, re-executing the compiled runbook must
//! produce the same sequence of verb calls with the same arguments.
//!
//! ## EnvelopeCore vs ReplayEnvelope
//!
//! `EnvelopeCore` contains only the deterministic fields that feed into the
//! content-addressed ID hash. Volatile fields like `sealed_at` and per-audit
//! timestamps are excluded from the hash input and live only in the full
//! `ReplayEnvelope` for audit purposes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use uuid::Uuid;

use crate::dsl_v2::macros::ExpansionLimits;

// ---------------------------------------------------------------------------
// EnvelopeCore — deterministic hash input (no timestamps)
// ---------------------------------------------------------------------------

/// The deterministic subset of `ReplayEnvelope` that feeds into the
/// content-addressed ID hash.
///
/// Excludes all volatile fields (timestamps) so that two compilations
/// of the same input at different times produce the same ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeCore {
    /// Session cursor at compilation time (monotonic sequence number).
    pub session_cursor: u64,

    /// Entity bindings resolved during compilation.
    ///
    /// Key: entity reference text (e.g., `"Allianz"`).
    /// Value: resolved entity UUID.
    ///
    /// Uses `BTreeMap` for deterministic serialization order (INV-2).
    pub entity_bindings: BTreeMap<String, Uuid>,

    /// SHA-256 digests of external lookup responses (deterministic, no timestamps).
    pub external_lookup_digests: Vec<String>,

    /// SHA-256 digests of macro expansion outputs (deterministic, no timestamps).
    pub macro_audit_digests: Vec<String>,

    /// Snapshot manifest: object_id → snapshot_id for every Semantic Registry
    /// snapshot consulted during compilation.
    ///
    /// This is the provenance chain that enables exact-point-in-time audit
    /// replay. Feeds into the content-addressed ID hash (INV-2).
    ///
    /// Empty when sem_reg is unavailable (graceful degradation).
    #[serde(default)]
    pub snapshot_manifest: HashMap<Uuid, Uuid>,
}

// ---------------------------------------------------------------------------
// ReplayEnvelope
// ---------------------------------------------------------------------------

/// Captures the non-deterministic inputs that were resolved at compile time.
///
/// Stored inside `CompiledRunbook` and never mutated after creation.
/// The `core` field contains the deterministic subset used for hashing;
/// the remaining fields are audit metadata (timestamps, full lookup records).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayEnvelope {
    /// Deterministic core — feeds into content-addressed ID hash.
    pub core: EnvelopeCore,

    /// External lookups performed during compilation (e.g., GLEIF, screening).
    /// Full records with timestamps for audit trail.
    pub external_lookups: Vec<ExternalLookup>,

    /// Macro expansion audits — one per macro expanded during compilation.
    /// Full records with timestamps for audit trail.
    pub macro_audits: Vec<MacroExpansionAudit>,

    /// When this envelope was sealed (audit only, not hashed).
    pub sealed_at: DateTime<Utc>,
}

impl ReplayEnvelope {
    /// Create an empty envelope (no external inputs).
    pub fn empty() -> Self {
        Self {
            core: EnvelopeCore {
                session_cursor: 0,
                entity_bindings: BTreeMap::new(),
                external_lookup_digests: Vec::new(),
                macro_audit_digests: Vec::new(),
                snapshot_manifest: HashMap::new(),
            },
            external_lookups: Vec::new(),
            macro_audits: Vec::new(),
            sealed_at: Utc::now(),
        }
    }

    /// Create an envelope with entity bindings.
    pub fn with_bindings(session_cursor: u64, bindings: BTreeMap<String, Uuid>) -> Self {
        Self {
            core: EnvelopeCore {
                session_cursor,
                entity_bindings: bindings,
                external_lookup_digests: Vec::new(),
                macro_audit_digests: Vec::new(),
                snapshot_manifest: HashMap::new(),
            },
            external_lookups: Vec::new(),
            macro_audits: Vec::new(),
            sealed_at: Utc::now(),
        }
    }

    /// Convenience accessor for session_cursor.
    pub fn session_cursor(&self) -> u64 {
        self.core.session_cursor
    }

    /// Convenience accessor for entity_bindings.
    pub fn entity_bindings(&self) -> &BTreeMap<String, Uuid> {
        &self.core.entity_bindings
    }
}

// ---------------------------------------------------------------------------
// ExternalLookup
// ---------------------------------------------------------------------------

/// Record of an external lookup performed during compilation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalLookup {
    /// Source system (e.g., `"gleif"`, `"screening"`, `"client_group"`).
    pub source: String,

    /// Query that was issued.
    pub query: String,

    /// SHA-256 digest of the response payload.
    pub response_digest: String,

    /// When the lookup was performed.
    pub performed_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// MacroExpansionAudit
// ---------------------------------------------------------------------------

/// Audit record for a single macro expansion during compilation.
///
/// Mirrors the existing `dsl_v2::macros::expander::MacroExpansionAudit` but
/// is owned by the runbook module to avoid cross-module coupling.
///
/// ## INV-12
///
/// `expansion_limits` captures the limits snapshot used during expansion
/// so that replay can verify the limits haven't changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroExpansionAudit {
    /// Unique expansion ID.
    pub expansion_id: Uuid,

    /// Macro fully-qualified name (e.g., `"structure.setup"`).
    pub macro_name: String,

    /// Parameters supplied to the macro.
    ///
    /// Uses `BTreeMap<String, String>` for deterministic serialization and
    /// guaranteed bincode round-trip (INV-2). `serde_json::Value` was removed
    /// because it can carry `f64` (violating no-floats) and bincode's internal
    /// `Value` encoding is not guaranteed stable across crate versions.
    pub params: BTreeMap<String, String>,

    /// Autofill values that were resolved from session state.
    ///
    /// Uses `BTreeMap<String, String>` for deterministic serialization (INV-2).
    pub resolved_autofill: BTreeMap<String, String>,

    /// SHA-256 digest of the expanded DSL output.
    pub expansion_digest: String,

    /// Expansion limits in effect during this expansion (INV-12).
    ///
    /// Captured so replay can verify the limits match. If limits change,
    /// the bincode layout changes → different content-addressed ID (INV-13).
    pub expansion_limits: ExpansionLimits,

    /// When the expansion was performed.
    pub expanded_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_envelope_round_trips() {
        let env = ReplayEnvelope::empty();
        let json = serde_json::to_string(&env).unwrap();
        let back: ReplayEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.core.session_cursor, 0);
        assert!(back.core.entity_bindings.is_empty());
        assert!(back.macro_audits.is_empty());
    }

    #[test]
    fn envelope_with_bindings() {
        let mut bindings = BTreeMap::new();
        bindings.insert("Allianz".into(), Uuid::new_v4());
        let env = ReplayEnvelope::with_bindings(42, bindings.clone());
        assert_eq!(env.core.session_cursor, 42);
        assert_eq!(env.core.entity_bindings.len(), 1);
    }

    #[test]
    fn convenience_accessors() {
        let mut bindings = BTreeMap::new();
        bindings.insert("Test".into(), Uuid::new_v4());
        let env = ReplayEnvelope::with_bindings(7, bindings);
        assert_eq!(env.session_cursor(), 7);
        assert_eq!(env.entity_bindings().len(), 1);
    }
}
