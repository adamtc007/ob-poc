//! Pack Operations — Journey pack lifecycle DSL verbs
//!
//! These operations make pack context derivable from the runbook fold:
//! - `pack.select` records which pack is active on the runbook
//! - `pack.answer` records Q&A answers from pack questions on the runbook
//!
//! Together with `session.load-cluster`, these ensure that ALL session state
//! can be reconstructed by folding over runbook entries (Invariant I-1).

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_macros::register_custom_op;
use serde::{Deserialize, Serialize};

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// RESULT TYPES
// =============================================================================

/// Result of selecting a journey pack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSelectResult {
    pub pack_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub manifest_hash: Option<String>,
    pub handoff_from: Option<String>,
}

/// Result of recording a pack answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackAnswerResult {
    pub field: String,
    pub value: String,
    pub accepted: bool,
    pub pack_id: Option<String>,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

// =============================================================================
// pack.select — Record pack selection on the runbook
// =============================================================================

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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let pack_id = get_required_string(verb_call, "pack-id")?;
        let pack_version =
            get_optional_string(verb_call, "pack-version").unwrap_or_else(|| "latest".to_string());
        let manifest_hash = get_optional_string(verb_call, "manifest-hash");
        let handoff_from = get_optional_string(verb_call, "handoff-from");

        // The pack selection itself is recorded by the RunbookEntry being on the runbook.
        // The ContextStack.from_runbook() fold reads entries with verb="pack.select"
        // and derives the active pack from the pack-id arg.
        //
        // This op returns the result for the entry's result field.
        let result = PackSelectResult {
            pack_id: pack_id.clone(),
            pack_name: pack_id.clone(), // Name resolved by orchestrator before calling
            pack_version,
            manifest_hash,
            handoff_from,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let pack_id = get_required_string(verb_call, "pack-id")?;
        let pack_version =
            get_optional_string(verb_call, "pack-version").unwrap_or_else(|| "latest".to_string());
        let manifest_hash = get_optional_string(verb_call, "manifest-hash");
        let handoff_from = get_optional_string(verb_call, "handoff-from");

        let result = PackSelectResult {
            pack_id: pack_id.clone(),
            pack_name: pack_id.clone(),
            pack_version,
            manifest_hash,
            handoff_from,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}

// =============================================================================
// pack.answer — Record a Q&A answer on the runbook
// =============================================================================

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

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let field = get_required_string(verb_call, "field")?;
        let value = get_required_string(verb_call, "value")?;
        let pack_id = get_optional_string(verb_call, "pack-id");

        // The answer is recorded by the RunbookEntry being on the runbook.
        // ContextStack.from_runbook() reads entries with verb="pack.answer"
        // and accumulates answers in the accumulated_answers map.
        let result = PackAnswerResult {
            field,
            value,
            accepted: true,
            pack_id,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        let field = get_required_string(verb_call, "field")?;
        let value = get_required_string(verb_call, "value")?;
        let pack_id = get_optional_string(verb_call, "pack-id");

        let result = PackAnswerResult {
            field,
            value,
            accepted: true,
            pack_id,
        };

        Ok(ExecutionResult::Record(serde_json::to_value(result)?))
    }
}
