//! Phrase authoring ops — 9 `phrase.*` verbs (Governed Phrase Authoring v1.2).
//!
//! Implements the YAML contracts in `config/verbs/phrase.yaml`. Each op is
//! a thin wrapper that dispatches to the [`PhraseService`] trait via
//! `ctx.service()` — the bridge keeps `crate::sem_reg::*` (snapshots,
//! ids, types) and the embedding-similarity SQL in ob-poc.
//!
//! Snapshot writes record `created_by = ctx.principal.actor_id`, so the
//! wrapper threads `&ctx.principal` into the dispatch call.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::PhraseService;

macro_rules! phrase_op {
    ($struct:ident, $verb:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct;

        #[async_trait]
        impl CustomOperation for $struct {
            fn domain(&self) -> &'static str {
                "phrase"
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
                let service = ctx.service::<dyn PhraseService>()?;
                let result = service
                    .dispatch_phrase_verb(pool, $verb, args, &ctx.principal)
                    .await?;
                Ok(VerbExecutionOutcome::Record(result))
            }
            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

phrase_op!(
    PhraseObserveMissesOp,
    "observe-misses",
    "Watermark-based incremental scan across session_traces with pattern aggregation"
);
phrase_op!(
    PhraseCoverageReportOp,
    "coverage-report",
    "Cross-join between dsl_verbs and verb_pattern_embeddings with domain-level aggregation"
);
phrase_op!(
    PhraseCheckCollisionsOp,
    "check-collisions",
    "Multi-source collision check: phrase_bank exact, embeddings exact, semantic similarity"
);
phrase_op!(
    PhraseProposeOp,
    "propose",
    "Proposal creation with collision check, risk tier assignment, and SemOS changeset wiring"
);
phrase_op!(
    PhraseBatchProposeOp,
    "batch-propose",
    "Batch proposal generation with per-phrase collision checks and risk tier aggregation"
);
phrase_op!(
    PhraseReviewProposalsOp,
    "review-proposals",
    "Multi-table join across proposals, collision reports, and risk tiers with grouping"
);
phrase_op!(
    PhraseApproveOp,
    "approve",
    "Approval requires SemOS changeset creation, phrase_bank insertion, and embedding generation"
);
phrase_op!(
    PhraseRejectOp,
    "reject",
    "Rejection requires state transition and audit trail recording"
);
phrase_op!(
    PhraseDeferOp,
    "defer",
    "Deferral requires state transition and optional reason recording"
);
