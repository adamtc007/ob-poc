//! Core types for cross-workspace state consistency.
//!
//! These types mirror the `shared_atom_registry` table and its lifecycle FSM.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Shared Atom Lifecycle ────────────────────────────────────────────

/// Lifecycle states for a shared atom (INV-6: if shared, always enforced once Active).
///
/// Draft → Active → Deprecated → Retired
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedAtomLifecycle {
    /// Declared but not enforced. Consumer discovery runs but supersession
    /// does not trigger propagation.
    Draft,
    /// Full enforcement. Every supersession triggers the three-stage
    /// propagation pipeline.
    Active,
    /// Still enforced — existing consumers protected. No new consumers allowed.
    Deprecated,
    /// Deregistered from active enforcement. Historical records retained.
    Retired,
}

impl SharedAtomLifecycle {
    /// Returns whether this lifecycle state triggers staleness propagation.
    pub fn triggers_propagation(&self) -> bool {
        matches!(self, Self::Active | Self::Deprecated)
    }

    /// Returns whether new consumers can be registered in this state.
    pub fn allows_new_consumers(&self) -> bool {
        matches!(self, Self::Draft | Self::Active)
    }

    /// Valid transitions from this state.
    pub fn valid_transitions(&self) -> &[SharedAtomLifecycle] {
        match self {
            Self::Draft => &[Self::Active],
            Self::Active => &[Self::Deprecated],
            Self::Deprecated => &[Self::Active, Self::Retired],
            Self::Retired => &[],
        }
    }

    /// Check if transitioning to `target` is allowed.
    pub fn can_transition_to(&self, target: SharedAtomLifecycle) -> bool {
        self.valid_transitions().contains(&target)
    }
}

// ── Shared Atom Definition ───────────────────────────────────────────

/// A shared atom declaration — an attribute whose value is owned by one workspace
/// but consumed by one or more other workspaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAtomDef {
    pub id: Uuid,
    pub atom_path: String,
    pub display_name: String,
    pub owner_workspace: String,
    pub owner_constellation_family: String,
    pub lifecycle_status: SharedAtomLifecycle,
    pub validation_rule: Option<SharedAtomValidation>,
    pub created_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Validation constraints for a shared atom value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAtomValidation {
    /// Regex pattern the value must match (e.g., `^[0-9A-Z]{20}$` for LEI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Enumerated set of allowed values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<String>>,

    /// Whether external verification is required (e.g., GLEIF for LEI).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gleif_verification: Option<String>,
}

// ── Registration Input ───────────────────────────────────────────────

/// Input for registering a new shared atom (enters Draft state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterSharedAtomInput {
    pub atom_path: String,
    pub display_name: String,
    pub owner_workspace: String,
    pub owner_constellation_family: String,
    pub validation_rule: Option<SharedAtomValidation>,
}

// ── Result types ─────────────────────────────────────────────────────

/// Result of a lifecycle transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTransitionResult {
    pub atom_id: Uuid,
    pub atom_path: String,
    pub from_status: SharedAtomLifecycle,
    pub to_status: SharedAtomLifecycle,
}

/// Summary row for listing shared atoms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAtomSummary {
    pub id: Uuid,
    pub atom_path: String,
    pub display_name: String,
    pub owner_workspace: String,
    pub lifecycle_status: SharedAtomLifecycle,
    pub created_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_transitions() {
        assert!(SharedAtomLifecycle::Draft.can_transition_to(SharedAtomLifecycle::Active));
        assert!(!SharedAtomLifecycle::Draft.can_transition_to(SharedAtomLifecycle::Retired));
        assert!(SharedAtomLifecycle::Active.can_transition_to(SharedAtomLifecycle::Deprecated));
        assert!(!SharedAtomLifecycle::Active.can_transition_to(SharedAtomLifecycle::Retired));
        assert!(SharedAtomLifecycle::Deprecated.can_transition_to(SharedAtomLifecycle::Active));
        assert!(SharedAtomLifecycle::Deprecated.can_transition_to(SharedAtomLifecycle::Retired));
        assert!(SharedAtomLifecycle::Retired.valid_transitions().is_empty());
    }

    #[test]
    fn propagation_triggers() {
        assert!(!SharedAtomLifecycle::Draft.triggers_propagation());
        assert!(SharedAtomLifecycle::Active.triggers_propagation());
        assert!(SharedAtomLifecycle::Deprecated.triggers_propagation());
        assert!(!SharedAtomLifecycle::Retired.triggers_propagation());
    }

    #[test]
    fn serde_roundtrip() {
        let input = RegisterSharedAtomInput {
            atom_path: "entity.lei".to_string(),
            display_name: "Legal Entity Identifier".to_string(),
            owner_workspace: "kyc".to_string(),
            owner_constellation_family: "kyc_workspace".to_string(),
            validation_rule: Some(SharedAtomValidation {
                format: Some("^[0-9A-Z]{20}$".to_string()),
                allowed_values: None,
                gleif_verification: Some("required".to_string()),
            }),
        };
        let json = serde_json::to_string(&input).unwrap();
        let back: RegisterSharedAtomInput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.atom_path, "entity.lei");
    }
}
