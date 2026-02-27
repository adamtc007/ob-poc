//! Lightweight mirrors of sem_os_core types for use in the proc macro.
//!
//! Proc-macro crates cannot depend on regular crates, so we maintain
//! minimal copies of the types needed for governance checks. These
//! are deserialized from the bincode cache file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Governance tier — mirrors `sem_os_core::types::GovernanceTier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceTier {
    Governed,
    Operational,
}

/// Trust class — mirrors `sem_os_core::types::TrustClass`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustClass {
    Proof,
    DecisionSupport,
    Convenience,
}

/// Snapshot status — mirrors `sem_os_core::types::SnapshotStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    Draft,
    Active,
    Deprecated,
    Retired,
}

/// Object type — mirrors `sem_os_core::types::ObjectType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
    AttributeDef,
    EntityTypeDef,
    RelationshipTypeDef,
    VerbContract,
    TaxonomyDef,
    TaxonomyNode,
    MembershipRule,
    ViewDef,
    PolicyRule,
    EvidenceRequirement,
    DocumentTypeDef,
    ObservationDef,
    DerivationSpec,
}

/// Classification level — mirrors `sem_os_core::types::Classification`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

/// A single entry in the governance cache.
///
/// Flattened from `SnapshotRow` — contains only the fields needed
/// for the 5 governance checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Fully-qualified name (e.g., "cbu.create", "cbu.jurisdiction_code")
    pub fqn: String,
    /// Object type discriminator
    pub object_type: ObjectType,
    /// Current lifecycle status
    pub status: SnapshotStatus,
    /// Governance tier
    pub governance_tier: GovernanceTier,
    /// Trust class
    pub trust_class: TrustClass,
    /// Whether this object carries PII
    pub pii: bool,
    /// Classification level
    pub classification: Classification,
}

/// The complete governance cache, serialized to bincode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernedCache {
    /// Cache format version (for forward compatibility)
    pub version: u32,
    /// When this cache was generated (ISO 8601)
    pub generated_at: String,
    /// Entries keyed by FQN for O(1) lookup
    pub entries: HashMap<String, CacheEntry>,
}

impl GovernedCache {
    pub const CURRENT_VERSION: u32 = 1;

    /// Look up a verb contract by FQN.
    pub fn lookup_verb(&self, fqn: &str) -> Option<&CacheEntry> {
        self.entries
            .get(fqn)
            .filter(|e| matches!(e.object_type, ObjectType::VerbContract))
    }

    /// Look up an attribute definition by FQN.
    pub fn lookup_attribute(&self, fqn: &str) -> Option<&CacheEntry> {
        self.entries
            .get(fqn)
            .filter(|e| matches!(e.object_type, ObjectType::AttributeDef))
    }
}
