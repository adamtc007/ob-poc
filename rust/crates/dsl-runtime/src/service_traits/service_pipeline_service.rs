//! Service-resource pipeline (intent → discovery → attribute rollup →
//! provisioning → readiness).
//!
//! 16 verbs across 7 domains (`service-intent.*`, `discovery.*`,
//! `attributes.*`, `provisioning.*`, `readiness.*`, `pipeline.full`,
//! `service-resource.*`) collapse onto a single dispatch method. The
//! ob-poc bridge wraps `crate::service_resources::*` (engines,
//! orchestrators, SRDEF registry loader) which stay in ob-poc.
//!
//! The trait returns [`crate::execution::VerbExecutionOutcome`]
//! directly because verbs here emit four shapes (`Uuid`, `Record`,
//! `RecordSet`, `Affected`) per their YAML `returns.type`. Round-tripping
//! through `serde_json::Value` would lose the variant tag.

use anyhow::Result;
use async_trait::async_trait;

use crate::execution::VerbExecutionOutcome;
use crate::tx::TransactionScope;

#[async_trait]
pub trait ServicePipelineService: Send + Sync {
    async fn dispatch_service_pipeline_verb(
        &self,
        scope: &mut dyn TransactionScope,
        domain: &str,
        verb_name: &str,
        args: &serde_json::Value,
    ) -> Result<VerbExecutionOutcome>;
}
