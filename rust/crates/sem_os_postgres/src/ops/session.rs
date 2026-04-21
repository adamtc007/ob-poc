//! Session verbs — SemOS-side YAML-first re-implementation of 19
//! `session.*` ops. All delegate to
//! [`SessionService::dispatch_session_verb`] which owns the
//! `UnifiedSession` mega-module in ob-poc. Pending session state
//! crosses turns via `ctx.extensions["_pending_session"]`. YAML
//! contracts in `config/verbs/session.yaml`.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::service_traits::SessionService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

macro_rules! session_op {
    ($struct:ident, $verb:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("session.", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let service = ctx.service::<dyn SessionService>()?;
                let result = service
                    .dispatch_session_verb(scope.pool(), $verb, args, &mut ctx.extensions)
                    .await?;
                Ok(VerbExecutionOutcome::Record(result))
            }
        }
    };
}

session_op!(Start, "start");
session_op!(LoadUniverse, "load-universe");
session_op!(LoadGalaxy, "load-galaxy");
session_op!(LoadCluster, "load-cluster");
session_op!(LoadSystem, "load-system");
session_op!(UnloadSystem, "unload-system");
session_op!(FilterJurisdiction, "filter-jurisdiction");
session_op!(Clear, "clear");
session_op!(Undo, "undo");
session_op!(Redo, "redo");
session_op!(Info, "info");
session_op!(List, "list");
session_op!(SetClient, "set-client");
session_op!(SetPersona, "set-persona");
session_op!(SetStructure, "set-structure");
session_op!(SetCase, "set-case");
session_op!(SetMandate, "set-mandate");
session_op!(LoadDeal, "load-deal");
session_op!(UnloadDeal, "unload-deal");
