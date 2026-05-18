//! Constellation hydration — Phase 3.2 (C-04).
//!
//! The Sage planning loop needs a snapshot of the session's current
//! constellation state (slot tree + entity states) to compute the
//! frontier (Phase 3.3) and detect blockers (Phase 3.4). Phase 3.2
//! defines the trait the loop calls through plus the in-memory DTO
//! the snapshot lives in.
//!
//! ## Spike scope (Phase 3.2)
//!
//! - Pure DTO + async trait. No transport binding to the SemOS
//!   substrate. The Phase 4 cutover swaps `StubConstellationHydrator`
//!   for a `SemOsMcpHydrator` once `sem_os_mcp` lands.
//! - Snapshot shape is intentionally narrow: the entity-state list
//!   (each carrying entity id, kind, lifecycle state, attribute
//!   bag) plus a `hydrated_at` timestamp. Phase 3.3 widens with
//!   slot-tree projection if needed; Phase 4 may project the full
//!   `HydratedSlot` tree.
//!
//! ## Charter discipline
//!
//! No `ob-poc` dep introduced — the snapshot DTO is defined here in
//! agent-side terms, not as a re-export of an `ob-poc`-resident
//! type. Phase 4 will define an MCP transport DTO in `sem_os_mcp`
//! and translate at the seam; the agent's planning loop never sees
//! the database-bound `EntityState` type from `ob-poc-sage`.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One entity's lifecycle state + attributes in the constellation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityStateDTO {
    /// Stable entity id (matches the `entity-type:uuid` token shape
    /// V&S §6.5 mentions; format is opaque to the planning loop).
    pub entity_id: String,
    /// Entity kind tag (`cbu`, `kyc_case`, `instrument`, …).
    pub entity_kind: String,
    /// Lifecycle state node (`draft`, `awaiting_kyc`, …). The active
    /// verb surface is gated against this.
    pub state: String,
    /// Free-form attribute bag — the substrate fills this from the
    /// scoped projection it sends. Values are typed at the consumer
    /// (`String`, `Number`, `Bool`, `Array`, `Object`) without
    /// further constraint at this layer.
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

/// In-memory constellation snapshot the planning loop reads.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConstellationSnapshot {
    /// All entities in the session's scope at hydration time.
    pub entity_states: Vec<EntityStateDTO>,
    /// When the snapshot was produced. Phase 4's dirty-flag
    /// refresh path consumes this to decide whether to re-hydrate.
    pub hydrated_at: DateTime<Utc>,
}

impl ConstellationSnapshot {
    /// Convenience constructor for the empty snapshot the spike
    /// hydrator returns.
    pub fn empty() -> Self {
        Self {
            entity_states: Vec::new(),
            hydrated_at: Utc::now(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entity_states.is_empty()
    }
}

/// Scope the hydrator resolves the snapshot against. Mirrors the
/// `ValidVerbSetScope` shape from `ob-poc-sage::engine` (which lives
/// behind the database feature) — phrased here in pure DTOs.
#[derive(Debug, Clone)]
pub struct HydrationScope<'a> {
    /// Workspace tag (`cbu`, `onboarding_request`, …).
    pub workspace: &'a str,
    /// Pack id the session anchored against.
    pub pack_id: &'a str,
    /// Optional constellation id the substrate scopes against.
    /// Phase 3.2 leaves this for the binary integrator; Phase 4
    /// threads the editor-supplied scope through.
    pub constellation_id: Option<&'a str>,
}

/// Async hydrator the planning loop calls to refresh its in-memory
/// constellation snapshot.
///
/// Two impls planned:
/// - [`StubConstellationHydrator`] — Phase 3.2 spike. Returns an
///   empty snapshot.
/// - `SemOsMcpHydrator` (Phase 4) — speaks MCP to `sem_os_mcp` for
///   real hydration.
#[async_trait]
pub trait ConstellationHydrator: Send + Sync {
    async fn hydrate(
        &self,
        scope: HydrationScope<'_>,
    ) -> Result<ConstellationSnapshot, HydrationError>;

    /// Provider label for diagnostics + audit.
    fn provider_label(&self) -> &str {
        "unknown"
    }
}

/// Errors produced by the hydrator. Narrow on purpose; Phase 4
/// widens for MCP transport failures.
#[derive(Debug, thiserror::Error)]
pub enum HydrationError {
    #[error("hydration unsupported: {0}")]
    Unsupported(String),
    #[error("hydration transport failure: {0}")]
    Transport(String),
}

/// Spike hydrator. Returns an empty snapshot for every scope and
/// records the call at debug level so the seam is observable.
#[derive(Debug, Default, Clone)]
pub struct StubConstellationHydrator {
    label: String,
}

impl StubConstellationHydrator {
    pub fn new() -> Self {
        Self {
            label: "stub".to_string(),
        }
    }

    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

#[async_trait]
impl ConstellationHydrator for StubConstellationHydrator {
    async fn hydrate(
        &self,
        scope: HydrationScope<'_>,
    ) -> Result<ConstellationSnapshot, HydrationError> {
        tracing::debug!(
            target: "sage-acp",
            workspace = scope.workspace,
            pack_id = scope.pack_id,
            constellation_id = ?scope.constellation_id,
            "stub constellation hydrator — returning empty snapshot"
        );
        Ok(ConstellationSnapshot::empty())
    }

    fn provider_label(&self) -> &str {
        &self.label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_hydrator_returns_empty_snapshot() {
        let hydrator = StubConstellationHydrator::new();
        let snapshot = hydrator
            .hydrate(HydrationScope {
                workspace: "cbu",
                pack_id: "book-setup",
                constellation_id: None,
            })
            .await
            .expect("stub never errors");
        assert!(snapshot.is_empty());
        assert_eq!(hydrator.provider_label(), "stub");
    }

    #[tokio::test]
    async fn stub_hydrator_label_override() {
        let hydrator = StubConstellationHydrator::with_label("phase-3-spike");
        assert_eq!(hydrator.provider_label(), "phase-3-spike");
    }

    #[test]
    fn empty_snapshot_round_trips_json() {
        let snapshot = ConstellationSnapshot::empty();
        let json = serde_json::to_value(&snapshot).unwrap();
        let parsed: ConstellationSnapshot = serde_json::from_value(json).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn snapshot_with_entities_round_trips_json() {
        let snapshot = ConstellationSnapshot {
            entity_states: vec![
                EntityStateDTO {
                    entity_id: "cbu:abc".to_string(),
                    entity_kind: "cbu".to_string(),
                    state: "draft".to_string(),
                    attributes: HashMap::from([(
                        "jurisdiction".to_string(),
                        serde_json::json!("LU"),
                    )]),
                },
                EntityStateDTO {
                    entity_id: "kyc_case:xyz".to_string(),
                    entity_kind: "kyc_case".to_string(),
                    state: "awaiting_docs".to_string(),
                    attributes: HashMap::new(),
                },
            ],
            hydrated_at: Utc::now(),
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        let parsed: ConstellationSnapshot = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.entity_states.len(), 2);
        assert_eq!(parsed.entity_states[0].entity_id, "cbu:abc");
        assert_eq!(
            parsed.entity_states[0]
                .attributes
                .get("jurisdiction")
                .unwrap(),
            "LU"
        );
    }
}
