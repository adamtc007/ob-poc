//! Observatory orientation types — WASM-safe version for frontend consumption.
//!
//! These types mirror `sem_os_core::observatory::orientation` but replace
//! `AgentMode` with `String` to avoid pulling sem_os_core into the WASM
//! build. Projection from internal types happens in sem_os_core's
//! observatory module.
//!
//! See THE_OBSERVATORY_v1.0.md §4.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::galaxy::ViewLevel;

// ── OrientationContract ──────────────────────────────────────

/// The Orientation Contract — present in every ShowPacket.
/// Canonical answer to the six orientation questions:
/// 1. What is in focus?  (focus_kind + focus_identity)
/// 2. At what level?     (view_level)
/// 3. What lens?         (lens)
/// 4. What mode?         (session_mode)
/// 5. Why am I here?     (entry_reason)
/// 6. What can I do?     (available_actions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrientationContract {
    /// Research | Governed | Maintenance (WASM-safe: String instead of AgentMode).
    pub session_mode: String,
    /// Universe | Cluster | System | Planet | Surface | Core
    pub view_level: ViewLevel,
    /// What kind of object is in focus.
    pub focus_kind: FocusKind,
    /// Identity of the focused object.
    pub focus_identity: FocusIdentity,
    /// Observation scope.
    pub scope: ObservatoryScope,
    /// Active lens (overlay, depth, clustering, filters).
    pub lens: LensState,
    /// Why the user arrived at this orientation.
    pub entry_reason: EntryReason,
    /// Valid actions from the current focus (from ContextResolution).
    pub available_actions: Vec<ActionDescriptor>,
    /// What changed since the previous orientation (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_from_previous: Option<OrientationDelta>,
    /// When this contract was computed.
    pub computed_at: DateTime<Utc>,
}

// ── FocusKind ────────────────────────────────────────────────

/// What kind of object is in focus.
/// Projects from `SubjectRef` variants + `ObjectRef.object_type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusKind {
    Cbu,
    Entity,
    Document,
    Case,
    Task,
    TaxonomyNode,
    ChangeSet,
    Guardrail,
    MaintenanceSession,
    Constellation,
    View,
    /// Catch-all for registry object types not yet enumerated.
    Other(String),
}

// ── FocusIdentity ────────────────────────────────────────────

/// Identity of the focused object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusIdentity {
    /// UUID or FQN.
    pub canonical_id: String,
    /// Human-readable business label, e.g. "Manco LU-001 — Allianz SICAV".
    pub business_label: String,
    /// Registry object type, e.g. "entity_type_def".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
}

// ── ObservatoryScope ─────────────────────────────────────────

/// Observation scope — projects from NavigationScope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservatoryScope {
    SingleObject,
    FilteredSet,
    GraphNeighbourhood,
    Cluster,
    Constellation,
    Universe,
}

// ── LensState ────────────────────────────────────────────────

/// Active lens state — overlay, depth probe, clustering, filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensState {
    /// ActiveOnly or DraftOverlay with changeset ID.
    pub overlay: OverlayState,
    /// Depth probe: ownership, control, services, documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth_probe: Option<DepthProbe>,
    /// How clusters are grouped.
    pub cluster_mode: ClusterMode,
    /// Active filter expressions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_filters: Vec<FilterExpression>,
}

/// Overlay state — mirrors OverlayMode from stewardship.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OverlayState {
    ActiveOnly,
    DraftOverlay { changeset_id: Uuid },
}

/// Depth probe type — mirrors DepthType from galaxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepthProbe {
    Ownership,
    Control,
    Services,
    Documents,
}

/// Cluster grouping mode — mirrors ClusterType from galaxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClusterMode {
    #[default]
    Jurisdiction,
    Client,
    Risk,
    Product,
}

/// A filter expression (key-value for now, extensible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterExpression {
    pub field: String,
    pub operator: String,
    pub value: serde_json::Value,
}

// ── EntryReason ──────────────────────────────────────────────

/// Why the user arrived at this orientation.
/// NOVEL type — no existing type records navigation cause.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryReason {
    /// User navigated directly (typed command, clicked breadcrumb).
    DirectNavigation,
    /// User accepted an agent suggestion.
    SuggestionAccepted {
        suggestion_id: String,
    },
    /// User drilled down from a parent object.
    DrillDown {
        from_level: ViewLevel,
        from_id: String,
    },
    /// A workflow step brought the user here.
    WorkflowStep {
        step_name: String,
    },
    /// Search result navigation.
    SearchResult {
        query: String,
    },
    /// Deep link (URL or external reference).
    DeepLink {
        uri: String,
    },
    /// Initial session entry (no prior orientation).
    SessionStart,
    /// History navigation (back/forward).
    HistoryReplay {
        direction: String,
    },
}

// ── ActionDescriptor ─────────────────────────────────────────

/// A valid action available from the current focus.
/// Projects from VerbCandidate / GroundedActionOption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDescriptor {
    /// Verb FQN (e.g. "kyc-case.create").
    pub action_id: String,
    /// Human-readable label.
    pub label: String,
    /// Action kind: "primitive", "macro", "navigation", "diagnostic".
    pub action_kind: String,
    /// Whether this action is currently executable.
    pub enabled: bool,
    /// Reason if disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    /// Confidence/rank score (0.0-1.0).
    #[serde(default)]
    pub rank_score: f64,
}

// ── OrientationDelta ─────────────────────────────────────────

/// What changed between two OrientationContracts.
/// Computed by diffing — no new data, pure function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrientationDelta {
    /// Whether the session mode changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode_changed: Option<ModeChange>,
    /// Whether the view level changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_changed: Option<LevelChange>,
    /// Whether the focus target changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_changed: Option<FocusChange>,
    /// Whether the lens changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens_changed: Option<LensChange>,
    /// Whether the scope changed.
    pub scope_changed: bool,
    /// Number of actions added/removed.
    pub actions_added: usize,
    pub actions_removed: usize,
    /// Human-readable summary of the transition.
    pub summary: String,
}

/// Mode transition detail (WASM-safe: String instead of AgentMode).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeChange {
    pub from: String,
    pub to: String,
}

/// Level transition detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelChange {
    pub from: ViewLevel,
    pub to: ViewLevel,
}

/// Focus transition detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusChange {
    pub from_kind: FocusKind,
    pub to_kind: FocusKind,
    pub from_label: String,
    pub to_label: String,
}

/// Lens transition detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensChange {
    pub overlay_changed: bool,
    pub depth_changed: bool,
    pub cluster_changed: bool,
    pub filters_changed: bool,
}

// ── Tests ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orientation_contract_serde() {
        let contract = OrientationContract {
            session_mode: "governed".into(),
            view_level: ViewLevel::System,
            focus_kind: FocusKind::Cbu,
            focus_identity: FocusIdentity {
                canonical_id: "cbu-001".into(),
                business_label: "Manco LU-001 — Allianz SICAV".into(),
                object_type: None,
            },
            scope: ObservatoryScope::Constellation,
            lens: LensState {
                overlay: OverlayState::ActiveOnly,
                depth_probe: None,
                cluster_mode: ClusterMode::Jurisdiction,
                active_filters: vec![],
            },
            entry_reason: EntryReason::DirectNavigation,
            available_actions: vec![ActionDescriptor {
                action_id: "cbu.read".into(),
                label: "Read CBU".into(),
                action_kind: "primitive".into(),
                enabled: true,
                disabled_reason: None,
                rank_score: 0.95,
            }],
            delta_from_previous: None,
            computed_at: Utc::now(),
        };

        let json = serde_json::to_string(&contract).unwrap();
        assert!(json.contains("Manco LU-001"));
        assert!(json.contains("governed"));
        assert!(json.contains("system"));

        let back: OrientationContract = serde_json::from_str(&json).unwrap();
        assert_eq!(back.view_level, ViewLevel::System);
        assert_eq!(back.focus_kind, FocusKind::Cbu);
        assert_eq!(back.session_mode, "governed");
    }

    #[test]
    fn test_entry_reason_serde() {
        let reason = EntryReason::DrillDown {
            from_level: ViewLevel::Cluster,
            from_id: "cluster-lu".into(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("drill_down"));
        assert!(json.contains("cluster"));

        let back: EntryReason = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, EntryReason::DrillDown { .. }));
    }

    #[test]
    fn test_overlay_state_serde() {
        let active = OverlayState::ActiveOnly;
        let json = serde_json::to_string(&active).unwrap();
        assert!(json.contains("active_only"));

        let draft = OverlayState::DraftOverlay {
            changeset_id: Uuid::nil(),
        };
        let json = serde_json::to_string(&draft).unwrap();
        assert!(json.contains("draft_overlay"));
    }

    #[test]
    fn test_mode_change_serde() {
        let change = ModeChange {
            from: "research".into(),
            to: "governed".into(),
        };
        let json = serde_json::to_string(&change).unwrap();
        assert!(json.contains("research"));
        assert!(json.contains("governed"));

        let back: ModeChange = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from, "research");
        assert_eq!(back.to, "governed");
    }
}
