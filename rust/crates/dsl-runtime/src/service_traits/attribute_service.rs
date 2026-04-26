//! Attribute lifecycle (define / inspect / deprecate / derived / bridge / ...).
//!
//! 16 verbs across 3 domains: 13 `attribute.*`, 2 `document.*`
//! (list-attributes + check-extraction-coverage), 1 `derivation.*`
//! (recompute-stale). The bridge stays in ob-poc because it pulls
//! `crate::sem_reg::derivation_spec`, `crate::sem_reg::store::SnapshotStore`,
//! `crate::sem_reg::types::*`, `crate::services::attribute_identity_service`,
//! and `crate::service_resources::PopulationEngine` — multi-consumer
//! sem_reg surfaces with no dsl-runtime analogue.
//!
//! # Why a wrapping outcome type
//!
//! The 3 `define*` verbs publish a snapshot AND need to bind
//! `@attribute` for downstream verbs. The trait can't touch
//! [`crate::execution::VerbExecutionContext::bind`] directly (it lives
//! in dsl-runtime and the bridge is in ob-poc), so the bridge returns
//! both the outcome AND any post-execution bindings the wrapper should
//! apply to `ctx`. The wrapper iterates `bindings` and calls
//! `ctx.bind(name, uuid)` for each before returning the outcome.

use anyhow::Result;
use async_trait::async_trait;
use sem_os_core::principal::Principal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::execution::VerbExecutionOutcome;

/// Result of a dispatched attribute verb. `outcome` is the standard
/// per-verb return; `bindings` is a flat list of `@symbol → UUID`
/// associations the consumer wrapper applies via `ctx.bind` before
/// returning the outcome.
pub struct AttributeDispatchOutcome {
    pub outcome: VerbExecutionOutcome,
    pub bindings: Vec<(String, Uuid)>,
}

#[async_trait]
pub trait AttributeService: Send + Sync {
    async fn dispatch_attribute_verb(
        &self,
        pool: &PgPool,
        domain: &str,
        verb_name: &str,
        args: &serde_json::Value,
        principal: &Principal,
    ) -> Result<AttributeDispatchOutcome>;
}
