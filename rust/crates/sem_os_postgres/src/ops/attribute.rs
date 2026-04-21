//! Attribute lifecycle verbs — SemOS-side YAML-first re-implementation.
//!
//! 16 verbs spread across `attribute.*`, `document.*` (two helpers),
//! and `derivation.*`. Every op dispatches to
//! [`AttributeService::dispatch_attribute_verb`] which returns an
//! `AttributeDispatchOutcome { outcome, bindings }`. The three
//! `define*` verbs return a Uuid AND bind `@attribute` for downstream
//! verbs — the wrapper applies the bindings before returning. YAML
//! contracts in `config/verbs/attribute.yaml` and
//! `config/verbs/observation/derivation.yaml`.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::service_traits::AttributeService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

macro_rules! attribute_op {
    ($struct:ident, $domain:literal, $verb:literal) => {
        pub struct $struct;

        #[async_trait]
        impl SemOsVerbOp for $struct {
            fn fqn(&self) -> &str {
                concat!($domain, ".", $verb)
            }
            async fn execute(
                &self,
                args: &serde_json::Value,
                ctx: &mut VerbExecutionContext,
                scope: &mut dyn TransactionScope,
            ) -> Result<VerbExecutionOutcome> {
                let service = ctx.service::<dyn AttributeService>()?;
                let result = service
                    .dispatch_attribute_verb(scope.pool(), $domain, $verb, args, &ctx.principal)
                    .await?;
                for (name, uuid) in result.bindings {
                    ctx.bind(&name, uuid);
                }
                Ok(result.outcome)
            }
        }
    };
}

attribute_op!(AttributeListSources, "attribute", "list-sources");
attribute_op!(AttributeListSinks, "attribute", "list-sinks");
attribute_op!(AttributeTraceLineage, "attribute", "trace-lineage");
attribute_op!(AttributeListByDocument, "attribute", "list-by-document");
attribute_op!(AttributeCheckCoverage, "attribute", "check-coverage");
attribute_op!(DocumentListAttributes, "document", "list-attributes");
attribute_op!(
    DocumentCheckExtractionCoverage,
    "document",
    "check-extraction-coverage"
);
attribute_op!(AttributeDefineGoverned, "attribute", "define");
attribute_op!(AttributeDefineInternal, "attribute", "define-internal");
attribute_op!(AttributeUpdateInternal, "attribute", "update-internal");
attribute_op!(AttributeDefineDerived, "attribute", "define-derived");
attribute_op!(AttributeSetEvidenceGrade, "attribute", "set-evidence-grade");
attribute_op!(AttributeDeprecate, "attribute", "deprecate");
attribute_op!(AttributeInspect, "attribute", "inspect");
attribute_op!(DerivationRecomputeStale, "derivation", "recompute-stale");
attribute_op!(AttributeBridgeToSemos, "attribute", "bridge-to-semos");
