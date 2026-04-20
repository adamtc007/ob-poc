//! Pack Operations — Journey pack lifecycle DSL verbs.
//!
//! `pack.select` and `pack.answer` record pack context on the runbook so all
//! session state can be reconstructed by folding over runbook entries
//! (Invariant I-1 — see `rust/docs/INVARIANT-VERIFICATION.md`).

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::domain_ops::helpers::{json_extract_string, json_extract_string_opt};
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSelectResult {
    pub pack_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub manifest_hash: Option<String>,
    pub handoff_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackAnswerResult {
    pub field: String,
    pub value: String,
    pub accepted: bool,
    pub pack_id: Option<String>,
}

#[register_custom_op]
pub struct PackSelectOp;

#[async_trait]
impl CustomOperation for PackSelectOp {
    fn domain(&self) -> &'static str {
        "pack"
    }

    fn verb(&self) -> &'static str {
        "select"
    }

    fn rationale(&self) -> &'static str {
        "Records pack selection on the runbook so session state is derivable from runbook fold"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let pack_id = json_extract_string(args, "pack-id")?;
        let pack_version =
            json_extract_string_opt(args, "pack-version").unwrap_or_else(|| "latest".to_string());
        let manifest_hash = json_extract_string_opt(args, "manifest-hash");
        let handoff_from = json_extract_string_opt(args, "handoff-from");

        let result = PackSelectResult {
            pack_id: pack_id.clone(),
            pack_name: pack_id,
            pack_version,
            manifest_hash,
            handoff_from,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

#[register_custom_op]
pub struct PackAnswerOp;

#[async_trait]
impl CustomOperation for PackAnswerOp {
    fn domain(&self) -> &'static str {
        "pack"
    }

    fn verb(&self) -> &'static str {
        "answer"
    }

    fn rationale(&self) -> &'static str {
        "Records pack Q&A answers on the runbook so accumulated answers are derivable from fold"
    }

    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<VerbExecutionOutcome> {
        let field = json_extract_string(args, "field")?;
        let value = json_extract_string(args, "value")?;
        let pack_id = json_extract_string_opt(args, "pack-id");

        let result = PackAnswerResult {
            field,
            value,
            accepted: true,
            pack_id,
        };

        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
