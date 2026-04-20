//! ob-poc impl of [`dsl_runtime::service_traits::StewardshipDispatch`].
//!
//! Bridges the plane-crossing `StewardshipDispatch` trait (defined in
//! `dsl-runtime`) to the concrete `dispatch_phase0_tool` +
//! `dispatch_phase1_tool` cascade in `crate::sem_reg::stewardship`.
//!
//! The dispatcher builds an `ActorContext` from the caller's principal,
//! assembles a `SemRegToolContext`, and tries phase 0 then phase 1. Unknown
//! tool names fall through as `Ok(None)` — matching the existing cascade
//! semantics that `delegate_to_stew_tool_json` relied on.

use async_trait::async_trait;
use sem_os_core::principal::Principal;
use sqlx::PgPool;

use dsl_runtime::service_traits::{StewardshipDispatch, StewardshipOutcome};

use crate::sem_reg::abac::ActorContext;
use crate::sem_reg::agent::mcp_tools::{dispatch_tool, SemRegToolContext, SemRegToolResult};
use crate::sem_reg::stewardship::{dispatch_phase0_tool, dispatch_phase1_tool};
use crate::sem_reg::types::Classification;

/// Concrete stewardship dispatcher backed by the in-process
/// `sem_reg::stewardship` module.
pub struct ObPocStewardshipDispatch {
    pool: PgPool,
}

impl ObPocStewardshipDispatch {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn actor_from_principal(principal: &Principal) -> ActorContext {
        ActorContext {
            actor_id: principal.actor_id.clone(),
            roles: if principal.is_admin() {
                vec!["admin".to_string(), "operator".to_string()]
            } else {
                vec!["operator".to_string()]
            },
            department: None,
            clearance: Some(Classification::Internal),
            jurisdictions: vec![],
        }
    }
}

#[async_trait]
impl StewardshipDispatch for ObPocStewardshipDispatch {
    async fn dispatch(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
        principal: &Principal,
    ) -> anyhow::Result<Option<StewardshipOutcome>> {
        let actor = Self::actor_from_principal(principal);
        let tool_ctx = SemRegToolContext {
            pool: &self.pool,
            actor: &actor,
            sem_os_service: None,
        };

        // Three-level cascade mirroring the previous `delegate_to_*` helpers:
        // stewardship phase 0 → stewardship phase 1 → general SemReg MCP.
        // The first two return `Option`; the general dispatcher always
        // produces a result but rejects unknown tool names via
        // `success=false`. We preserve the ob-poc-side cascade semantics
        // by checking phase 0/1 first; general MCP is the final arm.
        if let Some(result) = dispatch_phase0_tool(&tool_ctx, tool_name, args).await {
            return Ok(Some(to_outcome(result)));
        }
        if let Some(result) = dispatch_phase1_tool(&tool_ctx, tool_name, args).await {
            return Ok(Some(to_outcome(result)));
        }
        // General SemReg MCP dispatch handles governance.validate /
        // governance.dry-run (route to "sem_reg_validate_plan") and
        // any other non-stewardship tool the consumer ops call.
        let general = dispatch_tool(&tool_ctx, tool_name, args).await;
        Ok(Some(to_outcome(general)))
    }
}

fn to_outcome(result: SemRegToolResult) -> StewardshipOutcome {
    StewardshipOutcome {
        success: result.success,
        data: result.data,
        message: result.error,
    }
}
