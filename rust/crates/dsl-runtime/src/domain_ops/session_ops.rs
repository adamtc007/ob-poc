//! Session Operations — 19 `session.*` verbs.
//!
//! Implements the YAML contracts in `config/verbs/session.yaml`. Each op
//! is a thin wrapper that dispatches to the [`SessionService`] trait via
//! `ctx.service::<dyn SessionService>()` — the bridge handles all the
//! heavy lifting against `crate::session::UnifiedSession` in ob-poc (a
//! 10934 LOC multi-consumer session mega-module that stays put).
//!
//! # Astro Navigation Metaphor
//!
//!   Universe  = All regions the client operates in (global footprint)
//!   Galaxy    = Regional (LU, DE, IE) — may host multiple ManCos
//!   Cluster   = ManCo's controlled CBUs (gravitational grouping)
//!   System    = Single CBU (solar system container)
//!
//! Pending session state crosses turns through
//! `ctx.extensions["_pending_session"]` (mirrors the legacy
//! `ext_set_pending_session` helper).

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::SessionService;

async fn dispatch(
    ctx: &mut VerbExecutionContext,
    pool: &PgPool,
    verb: &'static str,
    args: &serde_json::Value,
) -> Result<VerbExecutionOutcome> {
    let service = ctx.service::<dyn SessionService>()?;
    let result = service
        .dispatch_session_verb(pool, verb, args, &mut ctx.extensions)
        .await?;
    Ok(VerbExecutionOutcome::Record(result))
}

macro_rules! session_op {
    ($struct:ident, $verb:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct;

        #[async_trait]
        impl CustomOperation for $struct {
            fn domain(&self) -> &'static str {
                "session"
            }
            fn verb(&self) -> &'static str {
                $verb
            }
            fn rationale(&self) -> &'static str {
                $rationale
            }
            async fn execute_json(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                pool: &PgPool,
            ) -> Result<VerbExecutionOutcome> {
                dispatch(ctx, pool, $verb, args).await
            }
            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

session_op!(SessionStartOp, "start", "Start a new session (no CBUs loaded)");
session_op!(
    SessionLoadUniverseOp,
    "load-universe",
    "Load all CBUs (optionally filtered by client group)"
);
session_op!(
    SessionLoadGalaxyOp,
    "load-galaxy",
    "Load all CBUs in a jurisdiction (regional view)"
);
session_op!(
    SessionLoadClusterOp,
    "load-cluster",
    "Load CBUs under a ManCo/governance controller"
);
session_op!(
    SessionLoadSystemOp,
    "load-system",
    "Load a single CBU into session scope"
);
session_op!(
    SessionUnloadSystemOp,
    "unload-system",
    "Remove a CBU from session scope"
);
session_op!(
    SessionFilterJurisdictionOp,
    "filter-jurisdiction",
    "Filter loaded scope to a specific jurisdiction"
);
session_op!(SessionClearOp, "clear", "Clear all CBUs from session scope");
session_op!(SessionUndoOp, "undo", "Undo the last session mutation");
session_op!(SessionRedoOp, "redo", "Redo a previously undone session mutation");
session_op!(SessionInfoOp, "info", "Get aggregate session scope info");
session_op!(SessionListOp, "list", "List loaded CBUs with optional filters");
session_op!(
    SessionSetClientOp,
    "set-client",
    "Bind a client group to the session context frame"
);
session_op!(
    SessionSetPersonaOp,
    "set-persona",
    "Bind a persona to the session context frame"
);
session_op!(
    SessionSetStructureOp,
    "set-structure",
    "Bind a structure (ManCo/Fund/SubFund/Entity) to the session context frame"
);
session_op!(
    SessionSetCaseOp,
    "set-case",
    "Bind a KYC case to the session context frame"
);
session_op!(
    SessionSetMandateOp,
    "set-mandate",
    "Bind a mandate to the session context frame"
);
session_op!(
    SessionLoadDealOp,
    "load-deal",
    "Bind a deal to the session context frame"
);
session_op!(
    SessionUnloadDealOp,
    "unload-deal",
    "Remove the deal binding from the session context frame"
);
