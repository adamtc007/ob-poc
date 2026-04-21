//! Service-resource pipeline ops — 16 verbs across 7 domains.
//!
//! Implements the YAML contracts in
//! `config/verbs/service-{intent,pipeline,resource,availability}.yaml`.
//! Each op is a thin wrapper that dispatches to the
//! [`ServicePipelineService`] trait via `ctx.service()` — the bridge
//! handles all the heavy lifting against `crate::service_resources::*`
//! (engines + orchestrators + SRDEF registry loader) in ob-poc.
//!
//! Multi-domain dispatch: the 16 verbs span 7 domains, so the macro
//! takes `($struct, $domain, $verb, $rationale)`.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use sqlx::PgPool;

use crate::custom_op::CustomOperation;
use crate::execution::{VerbExecutionContext, VerbExecutionOutcome};
use crate::service_traits::ServicePipelineService;

macro_rules! service_pipeline_op {
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
                let service = ctx.service::<dyn ServicePipelineService>()?;
                service
                    .dispatch_service_pipeline_verb(pool, $domain, $verb, args)
                    .await
            }
            fn is_migrated(&self) -> bool {
                true
            }
        }
    };
}

service_pipeline_op!(
    ServiceIntentCreateOp,
    "service-intent",
    "create",
    "Creates service intent record linking CBU to product+service"
);
service_pipeline_op!(
    ServiceIntentListOp,
    "service-intent",
    "list",
    "Lists all service intents for a CBU with enrichment"
);
service_pipeline_op!(
    ServiceIntentSupersedeOp,
    "service-intent",
    "supersede",
    "Creates new intent version, marks old as superseded"
);
service_pipeline_op!(
    DiscoveryRunOp,
    "discovery",
    "run",
    "Orchestrates SRDEF discovery from service intents"
);
service_pipeline_op!(
    DiscoveryExplainOp,
    "discovery",
    "explain",
    "Returns discovery reasons for audit/debugging"
);
service_pipeline_op!(
    AttributeRollupOp,
    "attributes",
    "rollup",
    "Merges attribute requirements from multiple SRDEFs"
);
service_pipeline_op!(
    AttributePopulateOp,
    "attributes",
    "populate",
    "Pulls values from entity, CBU, document sources"
);
service_pipeline_op!(
    AttributeGapsOp,
    "attributes",
    "gaps",
    "Queries gap view for missing required attributes"
);
service_pipeline_op!(
    AttributeSetOp,
    "attributes",
    "set",
    "Sets attribute value with evidence tracking"
);
service_pipeline_op!(
    ProvisioningRunOp,
    "provisioning",
    "run",
    "Orchestrates resource provisioning with dependency ordering"
);
service_pipeline_op!(
    ProvisioningStatusOp,
    "provisioning",
    "status",
    "Queries provisioning request with latest event"
);
service_pipeline_op!(
    ReadinessComputeOp,
    "readiness",
    "compute",
    "Computes 'good to transact' status per service"
);
service_pipeline_op!(
    ReadinessExplainOp,
    "readiness",
    "explain",
    "Returns blocking reasons for debugging/remediation"
);
service_pipeline_op!(
    PipelineFullOp,
    "pipeline",
    "full",
    "Orchestrates complete pipeline: discovery → provision → readiness"
);
service_pipeline_op!(
    ServiceResourceCheckAttributeGapsOp,
    "service-resource",
    "check-attribute-gaps",
    "Cross-references SRDEF attribute requirements against SemOS and attribute registry"
);
service_pipeline_op!(
    ServiceResourceSyncDefinitionsOp,
    "service-resource",
    "sync-definitions",
    "Loads SRDEF configs from disk and syncs them to the database"
);
