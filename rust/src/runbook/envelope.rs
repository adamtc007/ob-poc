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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ReplayEnvelope
// ---------------------------------------------------------------------------

/// Captures the non-deterministic inputs that were resolved at compile time.
///
/// Stored inside `CompiledRunbook` and never mutated after creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEnvelope {
    /// Session cursor at compilation time (monotonic sequence number).
    pub session_cursor: u64,

    /// Entity bindings resolved during compilation.
    ///
    /// Key: entity reference text (e.g., `"Allianz"`).
    /// Value: resolved entity UUID.
    pub entity_bindings: HashMap<String, Uuid>,

    /// External lookups performed during compilation (e.g., GLEIF, screening).
    pub external_lookups: Vec<ExternalLookup>,

    /// Macro expansion audits — one per macro expanded during compilation.
    pub macro_audits: Vec<MacroExpansionAudit>,

    /// When this envelope was sealed.
    pub sealed_at: DateTime<Utc>,
}

impl ReplayEnvelope {
    /// Create an empty envelope (no external inputs).
    pub fn empty() -> Self {
        Self {
            session_cursor: 0,
            entity_bindings: HashMap::new(),
            external_lookups: Vec::new(),
            macro_audits: Vec::new(),
            sealed_at: Utc::now(),
        }
    }

    /// Create an envelope with entity bindings.
    pub fn with_bindings(session_cursor: u64, bindings: HashMap<String, Uuid>) -> Self {
        Self {
            session_cursor,
            entity_bindings: bindings,
            external_lookups: Vec::new(),
            macro_audits: Vec::new(),
            sealed_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// ExternalLookup
// ---------------------------------------------------------------------------

/// Record of an external lookup performed during compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroExpansionAudit {
    /// Unique expansion ID.
    pub expansion_id: Uuid,

    /// Macro fully-qualified name (e.g., `"structure.setup"`).
    pub macro_name: String,

    /// Parameters supplied to the macro.
    pub params: HashMap<String, serde_json::Value>,

    /// Autofill values that were resolved from session state.
    pub resolved_autofill: HashMap<String, serde_json::Value>,

    /// SHA-256 digest of the expanded DSL output.
    pub expansion_digest: String,

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
        assert_eq!(back.session_cursor, 0);
        assert!(back.entity_bindings.is_empty());
        assert!(back.macro_audits.is_empty());
    }

    #[test]
    fn envelope_with_bindings() {
        let mut bindings = HashMap::new();
        bindings.insert("Allianz".into(), Uuid::new_v4());
        let env = ReplayEnvelope::with_bindings(42, bindings.clone());
        assert_eq!(env.session_cursor, 42);
        assert_eq!(env.entity_bindings.len(), 1);
    }
}
