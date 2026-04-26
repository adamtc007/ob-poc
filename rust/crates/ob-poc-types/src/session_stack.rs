use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::galaxy::ViewLevel;

/// Stable bridge DTO for session-stack state shared by ob-poc and BPMN-lite.
///
/// This is intentionally not a serialized mirror of `ReplSessionV2`. Only
/// execution-relevant identity, scope, workspace stack, and trace-lineage
/// inputs belong here.
///
/// This is a value type copied across the integration boundary. Each system
/// persists and mutates its own copy independently.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SessionStackState {
    pub session_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<SessionScopeState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_workspace: Option<SessionWorkspaceKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_stack: Vec<SessionStackFrame>,
    #[serde(default)]
    pub trace_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionScopeState {
    pub client_group_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_group_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionWorkspaceKind {
    ProductMaintenance,
    Deal,
    Cbu,
    Kyc,
    InstrumentMatrix,
    OnBoarding,
    SemOsMaintenance,
    LifecycleResources,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionSubjectKind {
    ClientGroup,
    Cbu,
    Deal,
    Case,
    Handoff,
    Matrix,
    Product,
    Service,
    Resource,
    Attribute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConstraintCascadeState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandate_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deal_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionStackFrame {
    pub workspace: SessionWorkspaceKind,
    pub constellation_family: String,
    pub constellation_map: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<SessionSubjectKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<Uuid>,
    pub pushed_at: DateTime<Utc>,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub writes_since_push: u32,
    #[serde(default)]
    pub is_peek: bool,
    #[serde(default)]
    pub constraints: ConstraintCascadeState,
    #[serde(default)]
    pub view_level: ViewLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_slot_path: Option<String>,
}
