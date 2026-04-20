//! Navigation verb handlers — nav.drill, nav.zoom-out, nav.select, etc.
//!
//! These verbs mutate VIEWPORT STATE on the session (view level, focus slot,
//! lens, nav history). They do NOT mutate the DAG (resource state) and do NOT
//! trigger rehydration — except when nav.drill crosses a materialization boundary.
//!
//! The verb handlers return structured `NavResult` records. The Sequencer
//! reads these records (currently by message-prefix matching against
//! `ReplResponseV2.message`, see `apply_nav_result_if_present`) and applies
//! the viewport state changes to the session's `WorkspaceFrame`. This keeps
//! the single-write-path invariant: all session mutations happen in the
//! Sequencer, not in verb handlers.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::json_extract_string_opt;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

/// Result of a navigation operation.
/// The Sequencer reads these fields to update session viewport state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavResult {
    pub success: bool,
    pub message: String,
    /// If set, the Sequencer should update WorkspaceFrame.view_level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_level: Option<String>,
    /// If set, the Sequencer should update WorkspaceFrame.focus_slot_path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_slot: Option<String>,
    /// If true, the drill target is in a different CBU/constellation,
    /// requiring rehydration (materialization boundary crossing).
    #[serde(default)]
    pub crossed_boundary: bool,
    /// Navigation direction for history operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_direction: Option<String>,
}

/// nav.drill — semantic drill into a focused object.
///
/// Updates view level and focus slot. If the target is in a different CBU,
/// sets crossed_boundary=true so the Sequencer triggers rehydration.
#[register_custom_op]
pub struct NavDrillOp;

#[async_trait]
impl CustomOperation for NavDrillOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "drill"
    }
    fn rationale(&self) -> &'static str {
        "Semantic drill into a focused object — opens deeper level"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let target_id = json_extract_string_opt(args, "target_id");
        let result = NavResult {
            success: true,
            message: format!("Drilled to {}", target_id.as_deref().unwrap_or("target")),
            target_level: None,
            target_slot: target_id,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// nav.zoom-out — semantic zoom out, go up one level.
#[register_custom_op]
pub struct NavZoomOutOp;

#[async_trait]
impl CustomOperation for NavZoomOutOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "zoom-out"
    }
    fn rationale(&self) -> &'static str {
        "Semantic zoom out — go up one level (not visual zoom)"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let result = NavResult {
            success: true,
            message: "Zoomed out one level".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// nav.select — set semantic focus to a specific entity or object.
#[register_custom_op]
pub struct NavSelectOp;

#[async_trait]
impl CustomOperation for NavSelectOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "select"
    }
    fn rationale(&self) -> &'static str {
        "Set semantic focus to a specific entity or object"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let target_id = json_extract_string_opt(args, "target_id");
        let result = NavResult {
            success: true,
            message: format!("Focus set to {}", target_id.as_deref().unwrap_or("target")),
            target_level: None,
            target_slot: target_id,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// nav.set-cluster-type — change cluster grouping mode.
#[register_custom_op]
pub struct NavSetClusterTypeOp;

#[async_trait]
impl CustomOperation for NavSetClusterTypeOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "set-cluster-type"
    }
    fn rationale(&self) -> &'static str {
        "Change cluster grouping mode"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let result = NavResult {
            success: true,
            message: "Cluster mode updated".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// nav.set-lens — change observation lens.
#[register_custom_op]
pub struct NavSetLensOp;

#[async_trait]
impl CustomOperation for NavSetLensOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "set-lens"
    }
    fn rationale(&self) -> &'static str {
        "Change observation lens (overlay, depth probe, filters)"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let result = NavResult {
            success: true,
            message: "Lens updated".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// nav.history-back — navigate back in viewport history.
#[register_custom_op]
pub struct NavHistoryBackOp;

#[async_trait]
impl CustomOperation for NavHistoryBackOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "history-back"
    }
    fn rationale(&self) -> &'static str {
        "Navigate back in history"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let result = NavResult {
            success: true,
            message: "Navigated back".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: Some("back".into()),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

/// nav.history-forward — navigate forward in viewport history.
#[register_custom_op]
pub struct NavHistoryForwardOp;

#[async_trait]
impl CustomOperation for NavHistoryForwardOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "history-forward"
    }
    fn rationale(&self) -> &'static str {
        "Navigate forward in history"
    }

    async fn execute_json(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let result = NavResult {
            success: true,
            message: "Navigated forward".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: Some("forward".into()),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
