//! Navigation verbs — SemOS-side YAML-first re-implementation.
//!
//! # Contracts (from `rust/config/verbs/navigation.yaml`)
//!
//! All seven verbs in the `nav.*` domain mutate viewport state on the
//! REPL session (view level, focus slot, lens, nav history). They do
//! NOT touch the DAG (resource state) and do NOT trigger rehydration.
//! No database effects — the scoped txn is accepted but unused.
//!
//! The Sequencer reads the returned record's `message` string via
//! prefix matching (see `Sequencer::apply_nav_result_if_present` in
//! `rust/src/sequencer.rs`) to decide what viewport state change to
//! apply. **The exact message prefixes are load-bearing** — they are
//! the runtime contract between verb and Sequencer. Legacy prefixes
//! (preserved here):
//!
//! - `"Drilled to <target>"`    — drill
//! - `"Zoomed out one level"`   — zoom-out (prefix `"Zoomed out"`)
//! - `"Focus set to <target>"`  — select
//! - `"Lens updated"`           — set-lens
//! - `"Cluster mode updated"`   — set-cluster-type
//! - `"Navigated back"`         — history-back
//! - `"Navigated forward"`      — history-forward
//!
//! Output JSON is the minimal shape the YAML contract declares —
//! `{success, message}` for most verbs, `{direction, success}` for
//! history-back/forward, `Void` for the two `side_effects: none`
//! verbs (set-cluster-type, set-lens).

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Serialize;

use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize)]
struct NavMessage {
    success: bool,
    message: String,
}

fn extract_required_string<'a>(
    args: &'a serde_json::Value,
    key: &str,
    fqn: &str,
) -> Result<&'a str> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("{fqn}: required arg `{key}` missing or not a string"))
}

// ── nav.drill ─────────────────────────────────────────────────────
pub struct Drill;

#[async_trait]
impl SemOsVerbOp for Drill {
    fn fqn(&self) -> &str {
        "nav.drill"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let target_id = extract_required_string(args, "target_id", self.fqn())?;
        let out = NavMessage {
            success: true,
            message: format!("Drilled to {target_id}"),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(out)?))
    }
}

// ── nav.zoom-out ──────────────────────────────────────────────────
pub struct ZoomOut;

#[async_trait]
impl SemOsVerbOp for ZoomOut {
    fn fqn(&self) -> &str {
        "nav.zoom-out"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let out = NavMessage {
            success: true,
            message: "Zoomed out one level".to_string(),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(out)?))
    }
}

// ── nav.select ────────────────────────────────────────────────────
pub struct Select;

#[async_trait]
impl SemOsVerbOp for Select {
    fn fqn(&self) -> &str {
        "nav.select"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let target_id = extract_required_string(args, "target_id", self.fqn())?;
        let out = NavMessage {
            success: true,
            message: format!("Focus set to {target_id}"),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(out)?))
    }
}

// ── nav.set-cluster-type ──────────────────────────────────────────
pub struct SetClusterType;

#[async_trait]
impl SemOsVerbOp for SetClusterType {
    fn fqn(&self) -> &str {
        "nav.set-cluster-type"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // YAML requires cluster_type — validate and discard (no state yet;
        // the Sequencer detects via the legacy "Cluster mode updated"
        // message prefix for future wiring, but YAML declares `void`).
        let _ = extract_required_string(args, "cluster_type", self.fqn())?;
        // Return a record (not Void) so the Sequencer's message-prefix
        // detection still works until viewport lens storage lands.
        let out = NavMessage {
            success: true,
            message: "Cluster mode updated".to_string(),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(out)?))
    }
}

// ── nav.set-lens ──────────────────────────────────────────────────
pub struct SetLens;

#[async_trait]
impl SemOsVerbOp for SetLens {
    fn fqn(&self) -> &str {
        "nav.set-lens"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // Optional args: overlay, depth_probe. Neither is consumed yet
        // (wired when LensState lands on WorkspaceFrame — Phase 4 of the
        // Observatory work). Emit the message the Sequencer listens for.
        let out = NavMessage {
            success: true,
            message: "Lens updated".to_string(),
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(out)?))
    }
}

// ── nav.history-back ──────────────────────────────────────────────
pub struct HistoryBack;

#[async_trait]
impl SemOsVerbOp for HistoryBack {
    fn fqn(&self) -> &str {
        "nav.history-back"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // YAML returns `{direction, success}`; the Sequencer detects the
        // op via `message.starts_with("Navigated back")`, so we populate
        // a `message` field alongside the YAML-declared ones. Extra
        // fields are ignored by JSON consumers and keep the runtime
        // contract intact.
        let rec = serde_json::json!({
            "direction": "back",
            "success": true,
            "message": "Navigated back",
        });
        Ok(VerbExecutionOutcome::Record(rec))
    }
}

// ── nav.history-forward ───────────────────────────────────────────
pub struct HistoryForward;

#[async_trait]
impl SemOsVerbOp for HistoryForward {
    fn fqn(&self) -> &str {
        "nav.history-forward"
    }
    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let rec = serde_json::json!({
            "direction": "forward",
            "success": true,
            "message": "Navigated forward",
        });
        Ok(VerbExecutionOutcome::Record(rec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sem_os_core::principal::Principal;

    struct NullScope;
    impl TransactionScope for NullScope {
        fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
            ob_poc_types::TransactionScopeId::new()
        }
        fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
            unreachable!("nav verbs should not touch the database")
        }
        fn pool(&self) -> &sqlx::PgPool {
            unreachable!("nav verbs do not need the pool")
        }
    }

    fn rec(outcome: VerbExecutionOutcome) -> serde_json::Value {
        match outcome {
            VerbExecutionOutcome::Record(v) => v,
            other => panic!("expected Record, got {other:?}"),
        }
    }

    async fn run_op(
        op: &dyn SemOsVerbOp,
        args: serde_json::Value,
    ) -> Result<VerbExecutionOutcome> {
        let mut ctx = VerbExecutionContext::new(Principal::system());
        let mut scope = NullScope;
        op.execute(&args, &mut ctx, &mut scope).await
    }

    #[tokio::test]
    async fn drill_formats_message_prefix() {
        let r = rec(run_op(&Drill, serde_json::json!({"target_id": "cbu-123"}))
            .await
            .unwrap());
        assert_eq!(r["message"], "Drilled to cbu-123");
        assert_eq!(r["success"], true);
    }

    #[tokio::test]
    async fn drill_requires_target_id() {
        let err = run_op(&Drill, serde_json::json!({})).await.unwrap_err();
        assert!(err.to_string().contains("target_id"));
    }

    #[tokio::test]
    async fn zoom_out_message() {
        let r = rec(run_op(&ZoomOut, serde_json::json!({})).await.unwrap());
        assert_eq!(r["message"], "Zoomed out one level");
    }

    #[tokio::test]
    async fn select_formats_message_prefix() {
        let r = rec(run_op(&Select, serde_json::json!({"target_id": "entity-1"}))
            .await
            .unwrap());
        assert_eq!(r["message"], "Focus set to entity-1");
    }

    #[tokio::test]
    async fn select_requires_target_id() {
        let err = run_op(&Select, serde_json::json!({})).await.unwrap_err();
        assert!(err.to_string().contains("target_id"));
    }

    #[tokio::test]
    async fn set_cluster_type_message() {
        let r = rec(
            run_op(&SetClusterType, serde_json::json!({"cluster_type": "jurisdiction"}))
                .await
                .unwrap(),
        );
        assert_eq!(r["message"], "Cluster mode updated");
    }

    #[tokio::test]
    async fn set_cluster_type_requires_cluster_type() {
        let err = run_op(&SetClusterType, serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("cluster_type"));
    }

    #[tokio::test]
    async fn set_lens_message() {
        let r = rec(run_op(&SetLens, serde_json::json!({})).await.unwrap());
        assert_eq!(r["message"], "Lens updated");
    }

    #[tokio::test]
    async fn history_back_direction_and_message() {
        let r = rec(run_op(&HistoryBack, serde_json::json!({})).await.unwrap());
        assert_eq!(r["direction"], "back");
        assert_eq!(r["success"], true);
        assert_eq!(r["message"], "Navigated back");
    }

    #[tokio::test]
    async fn history_forward_direction_and_message() {
        let r = rec(run_op(&HistoryForward, serde_json::json!({})).await.unwrap());
        assert_eq!(r["direction"], "forward");
        assert_eq!(r["message"], "Navigated forward");
    }
}
