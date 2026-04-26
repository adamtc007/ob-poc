//! Phrase authoring verbs — SemOS-side YAML-first re-implementation.
//!
//! All nine `phrase.*` verbs (Governed Phrase Authoring v1.2) dispatch
//! to [`PhraseService::dispatch_phrase_verb`], which owns the
//! collision-check / proposal / approval / phrase_bank mutation logic
//! as well as the embedding-similarity SQL. The service sits in
//! `dsl-runtime::service_traits` and is registered on
//! `VerbExecutionContext.services` by ob-poc at startup.
//!
//! The service still takes `&PgPool` (Phase 5a signature). We pass
//! [`TransactionScope::pool`] — a transitional accessor that returns
//! the pool the scope was opened on. Statements the service issues do
//! NOT participate in the scope's txn; that's fine for now because
//! phrase-authoring writes are scoped to their own service-internal
//! transactions. The accessor goes away in a follow-up slice once the
//! service adopts `&mut Transaction` directly.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::service_traits::PhraseService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

macro_rules! phrase_op {
    ($struct:ident, $verb:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!("phrase.", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let service = ctx.service::<dyn PhraseService>()?;
                let result = service
                    .dispatch_phrase_verb(scope.pool(), $verb, args, &ctx.principal)
                    .await?;
                Ok(VerbExecutionOutcome::Record(result))
            }
        }
    };
}

phrase_op!(ObserveMisses, "observe-misses");
phrase_op!(CoverageReport, "coverage-report");
phrase_op!(CheckCollisions, "check-collisions");
phrase_op!(Propose, "propose");
phrase_op!(BatchPropose, "batch-propose");
phrase_op!(ReviewProposals, "review-proposals");
phrase_op!(Approve, "approve");
phrase_op!(Reject, "reject");
phrase_op!(Defer, "defer");
