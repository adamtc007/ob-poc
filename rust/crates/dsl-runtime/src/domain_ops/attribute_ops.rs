//! Attribute lifecycle ops — 16 verbs across `attribute.*`, `document.*`,
//! and `derivation.*`.
//!
//! Implements the YAML contracts in `config/verbs/attribute.yaml`,
//! `config/verbs/observation/derivation.yaml`, and the document-level
//! attribute helpers. Each op dispatches to the [`AttributeService`]
//! trait via `ctx.service()` — the bridge keeps `crate::sem_reg::*`
//! and `crate::services::attribute_identity_service` in ob-poc.
//!
//! Three `define*` verbs return a Uuid AND bind `@attribute` for
//! downstream verbs. The bridge returns those bindings in
//! [`AttributeDispatchOutcome::bindings`]; the wrapper applies them
//! via `ctx.bind` before returning the outcome.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::AttributeService;

macro_rules! attribute_op {
    ($struct:ident, $domain:literal, $verb:literal, $rationale:literal) => {
        #[register_custom_op]
        pub struct $struct;

        #[async_trait]
        impl CustomOperation for $struct {
            fn domain(&self) -> &'static str {
                $domain
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
                let service = ctx.service::<dyn AttributeService>()?;
                let result = service
                    .dispatch_attribute_verb(pool, $domain, $verb, args, &ctx.principal)
                    .await?;
                for (name, uuid) in result.bindings {
                    ctx.bind(&name, uuid);
                }
                Ok(result.outcome)
            }
            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

attribute_op!(
    AttributeListSourcesOp,
    "attribute",
    "list-sources",
    "Requires join across attribute registry and document links with proof strength ordering"
);
attribute_op!(
    AttributeListSinksOp,
    "attribute",
    "list-sinks",
    "Requires join across attribute registry and document links for sink relationships"
);
attribute_op!(
    AttributeTraceLineageOp,
    "attribute",
    "trace-lineage",
    "Requires multiple queries to build complete attribute lineage including sources, sinks, and resource requirements"
);
attribute_op!(
    AttributeListByDocumentOp,
    "attribute",
    "list-by-document",
    "Requires join across document types, links, and attribute registry"
);
attribute_op!(
    AttributeCheckCoverageOp,
    "attribute",
    "check-coverage",
    "Requires comparison of required_attributes JSONB against actual document_attribute_links mappings"
);
attribute_op!(
    DocumentListAttributesOp,
    "document",
    "list-attributes",
    "Requires join across document types, links, and attribute registry"
);
attribute_op!(
    DocumentCheckExtractionCoverageOp,
    "document",
    "check-extraction-coverage",
    "Requires complex analysis of entity requirements vs available documents and their extraction capabilities"
);
attribute_op!(
    AttributeDefineGovernedOp,
    "attribute",
    "define",
    "Dual-writes operational attribute_registry and governed AttributeDef snapshots"
);
attribute_op!(
    AttributeDefineInternalOp,
    "attribute",
    "define-internal",
    "Lightweight internal attribute definition — operational tier, auto-approved"
);
attribute_op!(
    AttributeUpdateInternalOp,
    "attribute",
    "update-internal",
    "Lightweight metadata update for internal attributes — no changeset ceremony"
);
attribute_op!(
    AttributeDefineDerivedOp,
    "attribute",
    "define-derived",
    "Atomically publishes coupled AttributeDef and DerivationSpec snapshots"
);
attribute_op!(
    AttributeSetEvidenceGradeOp,
    "attribute",
    "set-evidence-grade",
    "Publishes a new governed AttributeDef version and keeps a linked DerivationSpec in sync"
);
attribute_op!(
    AttributeDeprecateOp,
    "attribute",
    "deprecate",
    "Soft-deprecates governed snapshots without deleting operational attribute rows"
);
attribute_op!(
    AttributeInspectOp,
    "attribute",
    "inspect",
    "Aggregates operational registry state, governed snapshots, derivation metadata, and usage counts"
);
attribute_op!(
    DerivationRecomputeStaleOp,
    "derivation",
    "recompute-stale",
    "Triggers batch recomputation of stale derived values"
);
attribute_op!(
    AttributeBridgeToSemosOp,
    "attribute",
    "bridge-to-semos",
    "Bulk-publishes SemOS snapshots for ungoverned store attributes"
);
