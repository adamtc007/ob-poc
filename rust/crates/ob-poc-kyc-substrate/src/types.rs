//! Primitive newtypes for the KYC/UBO substrate.
//!
//! Thin wrappers over `Uuid` give compile-time distinction between subject
//! roots, edges, persons, entities, and obligations — preventing the common
//! bug of passing an EdgeId where a PersonId is required.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Identity newtypes ─────────────────────────────────────────────────────────

/// The root entity whose ownership/control is being determined.
/// One ordered event stream per `SubjectId` (per-subject ordering domain, Q6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SubjectId(pub Uuid);

/// A typed control or economic-interest edge in the control graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId(pub Uuid);

/// A natural person who may be a determined UBO or controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PersonId(pub Uuid);

/// Any legal entity (intermediate node, company, fund, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Uuid);

/// A KYC obligation (role + subject + jurisdiction + … per K-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObligationId(pub Uuid);

/// Primary key of an intent event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventId(pub Uuid);

impl EventId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

// ── Verb ──────────────────────────────────────────────────────────────────────

/// Fully-qualified verb name, e.g. `"ubo.edge.verify"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VerbFqn(pub String);

impl VerbFqn {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for VerbFqn {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::fmt::Display for VerbFqn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Cryptographic hash ────────────────────────────────────────────────────────

/// SHA-256 content hash (32 bytes). Used for lexicon entries, graph hash,
/// payload hash, and the `DeterminationPin`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(bytes);
        Self(h.finalize().into())
    }

    pub fn of_json(v: &serde_json::Value) -> Self {
        Self::of(v.to_string().as_bytes())
    }

    /// Hex representation (lowercase, 64 chars) — the on-the-wire / DB-column form.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse a 64-char lowercase hex string back into a `Hash`.
    ///
    /// Round-trips with [`Self::to_hex`] / [`std::fmt::Display`]. Used by the
    /// durable store to rehydrate `lexicon_hash` / `payload_hash` text columns.
    pub fn from_hex(s: &str) -> Result<Self, String> {
        let bytes = hex::decode(s).map_err(|e| format!("invalid hex: {e}"))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|v: Vec<u8>| format!("expected 32 bytes, got {}", v.len()))?;
        Ok(Self(arr))
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", hex::encode(self.0))
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

// ── Actor / authority ─────────────────────────────────────────────────────────

/// Thin representation of an invoking actor.  In the slice this is a role
/// string; in W1-proper it will be linked to the ABAC principal model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub actor_id: Uuid,
    pub role: String,
}

impl Principal {
    pub fn new(actor_id: Uuid, role: impl Into<String>) -> Self {
        Self { actor_id, role: role.into() }
    }

    /// Convenience: a fixed analyst principal for tests.
    pub fn test_analyst() -> Self {
        Self::new(
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            "analyst",
        )
    }
}

/// Object-capability reference that authorised this verb invocation (K-17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityRef(pub String);

// ── Target binding ────────────────────────────────────────────────────────────

/// What the verb acts on.  Not all fields are present for every verb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TargetBinding {
    pub subject_root: Option<SubjectId>,
    pub edge_id: Option<EdgeId>,
    pub entity_id: Option<EntityId>,
    pub person_id: Option<PersonId>,
    pub obligation_id: Option<ObligationId>,
}

impl TargetBinding {
    pub fn for_subject(id: SubjectId) -> Self {
        Self { subject_root: Some(id), ..Default::default() }
    }

    pub fn for_edge(subject: SubjectId, edge: EdgeId) -> Self {
        Self { subject_root: Some(subject), edge_id: Some(edge), ..Default::default() }
    }
}

// ── Idempotency key ───────────────────────────────────────────────────────────

/// Deduplication key.  Two events with the same key are the same logical
/// invocation and the second append is a no-op.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdemKey(pub String);

impl IdemKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn from_uuid(u: Uuid) -> Self {
        Self(u.to_string())
    }
}
