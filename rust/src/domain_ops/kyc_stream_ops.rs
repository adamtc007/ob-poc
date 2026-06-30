//! Stream-backed KYC/UBO determination verbs (EOP-DD-KYCUBO-002, rip-and-replace R1+).
//!
//! These are the `dsl.kyc` lexicon verbs — the V&S determination vocabulary that
//! REPLACES the legacy `ubo.*` / `control.*` / `ownership.*` / `board.*` write
//! verbs. Each op holds **no** determination logic: it builds an `IntentEvent`
//! from the verb args + the frozen execution identity (via the seam) and appends
//! it to the durable verb stream. Current state is a fold/projection of that
//! stream — never a direct table write.

use std::sync::{Arc, LazyLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use sem_os_postgres::ops::SemOsVerbOp;

use ob_poc_kyc_seam::{append_in_scope, IntentEventDraft};
use ob_poc_kyc_substrate::fold::control::check_control_preconditions;
use ob_poc_kyc_substrate::{
    phase1_lexicon, AuthorityRef, EdgeId, FoldRegistry, SubjectId, TargetBinding, V1FoldImpl,
};

use super::helpers::{json_extract_uuid, json_extract_uuid_opt};

/// The KYC fold registry (v1) — maps the phase-1 lexicon hash to its `FoldImpl`.
/// Module-level for now; becomes an injected platform service when fold
/// version-dispatch (D2) needs more than one registered version.
static KYC_REGISTRY: LazyLock<FoldRegistry> = LazyLock::new(|| {
    let mut registry = FoldRegistry::new();
    registry.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    registry
});

/// `ubo.edge.assert-control` — claim a control edge (voting, board, GP statutory,
/// …). The first stream-backed determination verb; the pattern every other
/// `dsl.kyc` verb follows.
pub struct UboEdgeAssertControl;

#[async_trait]
impl SemOsVerbOp for UboEdgeAssertControl {
    fn fqn(&self) -> &str {
        "ubo.edge.assert-control"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // The determination root this edge belongs to (the subject stream).
        let subject = SubjectId(json_extract_uuid(args, ctx, "subject-id")?);
        // Stable edge id (caller-supplied or fresh); the edge's identity in the fold.
        let edge = EdgeId(json_extract_uuid_opt(args, ctx, "edge-id").unwrap_or_else(Uuid::new_v4));

        let lexicon = phase1_lexicon();
        let entry = lexicon
            .get("ubo.edge.assert-control")
            .ok_or_else(|| anyhow!("ubo.edge.assert-control missing from lexicon"))?;

        // The verb args ARE the event payload (from_entity_id, to_entity_id,
        // edge_kind, percentage, …) — the fold reads them.
        let event = IntentEventDraft {
            verb_fqn: "ubo.edge.assert-control".into(),
            subject_root: subject,
            target: TargetBinding::for_edge(subject, edge),
            payload: args.clone(),
            authority: AuthorityRef("analyst.assert-control".into()),
            lexicon_hash: lexicon.hash,
            as_of: ctx.as_of, // frozen at verb entry — never now() here
        }
        .into_event(&ctx.principal, ctx.correlation_id, ctx.execution_id);

        let outcome = append_in_scope(scope, &KYC_REGISTRY, &event, |state| {
            check_control_preconditions(entry, state, &event)
        })
        .await
        .map_err(|e| anyhow!("ubo.edge.assert-control append failed: {e}"))?;

        Ok(VerbExecutionOutcome::Record(json!({
            "edge_id": edge.0,
            "seq": outcome.seq,
            "deduped": outcome.deduped,
        })))
    }
}
