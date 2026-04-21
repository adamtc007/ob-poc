//! Service-resource pipeline verbs — SemOS-side YAML-first re-implementation.
//!
//! 16 verbs spanning 7 domains (service-intent, discovery, attributes,
//! provisioning, readiness, pipeline, service-resource). Every op
//! dispatches to [`ServicePipelineService::dispatch_service_pipeline_verb`],
//! which orchestrates SRDEF engines in ob-poc. The service returns
//! `VerbExecutionOutcome` directly — no bindings, no wrapping. YAML
//! contracts in `config/verbs/service-{intent,pipeline,resource,availability}.yaml`.

use anyhow::Result;
use async_trait::async_trait;

use dsl_runtime::service_traits::ServicePipelineService;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

macro_rules! service_pipeline_op {
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
                let service = ctx.service::<dyn ServicePipelineService>()?;
                service
                    .dispatch_service_pipeline_verb(scope.pool(), $domain, $verb, args)
                    .await
            }
        }
    };
}

service_pipeline_op!(ServiceIntentCreate, "service-intent", "create");
service_pipeline_op!(ServiceIntentList, "service-intent", "list");
service_pipeline_op!(ServiceIntentSupersede, "service-intent", "supersede");
service_pipeline_op!(DiscoveryRun, "discovery", "run");
service_pipeline_op!(DiscoveryExplain, "discovery", "explain");
service_pipeline_op!(AttributeRollup, "attributes", "rollup");
service_pipeline_op!(AttributePopulate, "attributes", "populate");
service_pipeline_op!(AttributeGaps, "attributes", "gaps");
service_pipeline_op!(AttributeSet, "attributes", "set");
service_pipeline_op!(ProvisioningRun, "provisioning", "run");
service_pipeline_op!(ProvisioningStatus, "provisioning", "status");
service_pipeline_op!(ReadinessCompute, "readiness", "compute");
service_pipeline_op!(ReadinessExplain, "readiness", "explain");
service_pipeline_op!(PipelineFull, "pipeline", "full");
service_pipeline_op!(
    ServiceResourceCheckAttributeGaps,
    "service-resource",
    "check-attribute-gaps"
);
service_pipeline_op!(
    ServiceResourceSyncDefinitions,
    "service-resource",
    "sync-definitions"
);
