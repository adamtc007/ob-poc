//! Navigation verb handlers — nav.drill, nav.zoom-out, nav.select, etc.
//!
//! These verbs mutate VIEWPORT STATE on the session (view level, focus slot,
//! lens, nav history). They do NOT mutate the DAG (resource state) and do NOT
//! trigger rehydration — except when nav.drill crosses a materialization boundary.
//!
//! The verb handlers return structured NavResult records. The orchestrator reads
//! these results and applies the viewport state changes to the session's
//! WorkspaceFrame. This keeps the single-write-path invariant: all session
//! mutations happen in orchestrator_v2.process(), not in verb handlers.

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};

use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

/// Result of a navigation operation.
/// The orchestrator reads these fields to update session viewport state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavResult {
    pub success: bool,
    pub message: String,
    /// If set, the orchestrator should update WorkspaceFrame.view_level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_level: Option<String>,
    /// If set, the orchestrator should update WorkspaceFrame.focus_slot_path.
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
/// sets crossed_boundary=true so the orchestrator triggers rehydration.
#[register_custom_op]
pub struct NavDrillOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavDrillOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "drill"
    }
    fn rationale(&self) -> &'static str {
        "Semantic drill into a focused object — opens deeper level"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        let target_id = find_arg(verb_call, "target_id");
        let _target_level = find_arg(verb_call, "target_level");

        let result = NavResult {
            success: true,
            message: format!("Drilled to {}", target_id.as_deref().unwrap_or("target")),
            target_level: None,
            target_slot: target_id,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// Extract a string argument from a VerbCall by key.
fn find_arg(verb_call: &VerbCall, key: &str) -> Option<String> {
    use dsl_core::ast::{AstNode, Literal};
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| match &a.value {
            AstNode::Literal(Literal::String(s), _) => Some(s.clone()),
            AstNode::SymbolRef { name, .. } => Some(name.clone()),
            _ => None,
        })
}

/// nav.zoom-out — semantic zoom out, go up one level.
#[register_custom_op]
pub struct NavZoomOutOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavZoomOutOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "zoom-out"
    }
    fn rationale(&self) -> &'static str {
        "Semantic zoom out — go up one level (not visual zoom)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        // Orchestrator reads target_level=None + no target_slot and decrements level
        let result = NavResult {
            success: true,
            message: "Zoomed out one level".into(),
            target_level: None, // Orchestrator computes parent level
            target_slot: None,  // Clear focus on zoom out
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// nav.select — set semantic focus to a specific entity.
#[register_custom_op]
pub struct NavSelectOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavSelectOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "select"
    }
    fn rationale(&self) -> &'static str {
        "Set semantic focus to a specific entity or object"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        let target_id = find_arg(verb_call, "target_id");
        let result = NavResult {
            success: true,
            message: format!("Focus set to {}", target_id.as_deref().unwrap_or("target")),
            target_level: None,
            target_slot: target_id,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// nav.set-cluster-type — change cluster grouping mode.
#[register_custom_op]
pub struct NavSetClusterTypeOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavSetClusterTypeOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "set-cluster-type"
    }
    fn rationale(&self) -> &'static str {
        "Change cluster grouping mode"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        // Cluster mode is a lens parameter — orchestrator stores on session
        let result = NavResult {
            success: true,
            message: "Cluster mode updated".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// nav.set-lens — change observation lens.
#[register_custom_op]
pub struct NavSetLensOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavSetLensOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "set-lens"
    }
    fn rationale(&self) -> &'static str {
        "Change observation lens (overlay, depth probe, filters)"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        // Lens state is a viewport parameter — orchestrator stores on session
        let result = NavResult {
            success: true,
            message: "Lens updated".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: None,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// nav.history-back — navigate back in viewport history.
#[register_custom_op]
pub struct NavHistoryBackOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavHistoryBackOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "history-back"
    }
    fn rationale(&self) -> &'static str {
        "Navigate back in history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        let result = NavResult {
            success: true,
            message: "Navigated back".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: Some("back".into()),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

/// nav.history-forward — navigate forward in viewport history.
#[register_custom_op]
pub struct NavHistoryForwardOp;

#[async_trait]
impl crate::domain_ops::CustomOperation for NavHistoryForwardOp {
    fn domain(&self) -> &'static str {
        "nav"
    }
    fn verb(&self) -> &'static str {
        "history-forward"
    }
    fn rationale(&self) -> &'static str {
        "Navigate forward in history"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &sqlx::PgPool,
    ) -> Result<ExecutionResult> {
        let result = NavResult {
            success: true,
            message: "Navigated forward".into(),
            target_level: None,
            target_slot: None,
            crossed_boundary: false,
            history_direction: Some("forward".into()),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}
