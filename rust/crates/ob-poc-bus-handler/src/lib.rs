//! ob-poc receiver-side bus handler.
//!
//! Implements [`dsl_bus_server::InvocationDispatcher`] for the ob-poc
//! domain. The actual verb execution is delegated to a caller-supplied
//! [`VerbExecutor`] port so this crate stays free of the full ob-poc
//! engine surface. T2B.9 wires the real executor at app startup.
//!
//! ob-poc never receives results across the bus (it's a service domain,
//! not a workflow domain), so the matching [`NoopResultDispatcher`]
//! returns `BusServerError::UnknownVerb` for every DeliverResult call —
//! handy for `BusServer::builder().result_dispatcher(NoopResultDispatcher)`.
#![deny(unreachable_pub)]
#![forbid(unsafe_code)]

use std::sync::Arc;

use async_trait::async_trait;
use dsl_bus_protocol::v1::{ExecutionOutcome, ExecutionOutcomeKind, ResolvedBinding};
use dsl_bus_server::{
    BusServerError, InvocationContext, InvocationDispatcher, InvocationOutcome, ResultContext,
    ResultDispatcher,
};
use thiserror::Error;
use uuid::Uuid;

/// Successful local execution of an ob-poc verb. Translated by
/// [`ObPocBusHandler`] into the [`InvocationOutcome`] the bus server
/// returns to the caller.
#[derive(Debug, Clone)]
pub struct VerbOutcome {
    pub execution_id: Uuid,
    pub kind: ExecutionOutcomeKind,
    pub detail: String,
    pub bindings: Vec<ResolvedBinding>,
}

/// Failure modes a `VerbExecutor` can surface. Map onto the rejection
/// branches of `BusServerError` so the bus client sees the right
/// `SubmissionStatus` code.
#[derive(Debug, Error)]
pub enum VerbExecutorError {
    #[error("unknown verb: {0}")]
    UnknownVerb(String),

    #[error("catalogue version incompatible: {0}")]
    VersionIncompatible(String),

    #[error("authority denied: {0}")]
    AuthorityDenied(String),

    #[error("malformed input: {0}")]
    Malformed(String),

    #[error("internal engine error: {0}")]
    Internal(String),
}

impl From<VerbExecutorError> for BusServerError {
    fn from(e: VerbExecutorError) -> Self {
        match e {
            VerbExecutorError::UnknownVerb(s) => BusServerError::UnknownVerb(s),
            VerbExecutorError::VersionIncompatible(s) => BusServerError::VersionIncompatible(s),
            VerbExecutorError::AuthorityDenied(s) => BusServerError::AuthorityDenied(s),
            VerbExecutorError::Malformed(s) => BusServerError::Malformed(s),
            VerbExecutorError::Internal(s) => BusServerError::Internal(s),
        }
    }
}

/// Port the app supplies — the real implementation in T2B.9 calls
/// `dsl-runtime::execute_verb_sync` against the ob-poc engine.
#[async_trait]
pub trait VerbExecutor: Send + Sync + 'static {
    async fn execute(
        &self,
        local_verb_id: &str,
        catalogue_version: &str,
        inputs: Vec<ResolvedBinding>,
    ) -> Result<VerbOutcome, VerbExecutorError>;
}

/// `InvocationDispatcher` implementation for ob-poc. Holds an `Arc`
/// over the caller's [`VerbExecutor`] so a single instance can be
/// shared across the tonic server's worker threads.
pub struct ObPocBusHandler {
    executor: Arc<dyn VerbExecutor>,
    /// Configured catalogue version. When set, the dispatcher rejects
    /// any `InvocationRequest` whose `catalogue_version` doesn't match
    /// — T2B master DoD scenario #46 (version mismatch).
    expected_catalogue_version: Option<String>,
}

impl ObPocBusHandler {
    pub fn new<E: VerbExecutor>(executor: E) -> Self {
        Self {
            executor: Arc::new(executor),
            expected_catalogue_version: None,
        }
    }

    pub fn from_arc(executor: Arc<dyn VerbExecutor>) -> Self {
        Self {
            executor,
            expected_catalogue_version: None,
        }
    }

    /// Enforce that incoming invocations carry the named
    /// `catalogue_version`. Mismatches are rejected as
    /// `BusServerError::VersionIncompatible` (mapped on the wire to
    /// `SubmissionStatus::RejectedVersionIncompatible`).
    pub fn with_catalogue_version(mut self, version: impl Into<String>) -> Self {
        self.expected_catalogue_version = Some(version.into());
        self
    }
}

#[async_trait]
impl InvocationDispatcher for ObPocBusHandler {
    async fn dispatch(
        &self,
        ctx: InvocationContext,
        inputs: Vec<ResolvedBinding>,
    ) -> Result<InvocationOutcome, BusServerError> {
        if let Some(ref expected) = self.expected_catalogue_version {
            if &ctx.catalogue_version != expected {
                return Err(BusServerError::VersionIncompatible(format!(
                    "ob-poc expects catalogue_version {expected}, got {}",
                    ctx.catalogue_version
                )));
            }
        }
        let outcome = self
            .executor
            .execute(&ctx.local_verb_id, &ctx.catalogue_version, inputs)
            .await?;
        Ok(InvocationOutcome {
            execution_id: outcome.execution_id,
            outcome: ExecutionOutcome {
                kind: outcome.kind as i32,
                detail: outcome.detail,
                bindings: outcome.bindings,
            },
        })
    }
}

/// A3 §3.3 — ob-poc-branded re-exports of the canonical `EntityService`
/// / `SemOsService` stubs. The bus server registers these by name when
/// the manifest declares the corresponding capability. v0.6 stubs
/// return `NOT_IMPLEMENTED` per A3 §6 discipline #1.
pub use dsl_bus_server::StubEntityService as ObPocEntityServiceImpl;
pub use dsl_bus_server::StubSemOsService as ObPocSemOsServiceImpl;

/// `ResultDispatcher` for service-side domains that never receive
/// results. Always returns `BusServerError::UnknownVerb`, which the
/// server translates to `ReceiptStatus::RejectedUnknownExecution`.
pub struct NoopResultDispatcher;

#[async_trait]
impl ResultDispatcher for NoopResultDispatcher {
    async fn dispatch(
        &self,
        _ctx: ResultContext,
        _outcome: ExecutionOutcome,
    ) -> Result<(), BusServerError> {
        Err(BusServerError::UnknownVerb(
            "ob-poc does not receive bus results".into(),
        ))
    }
}

#[cfg(test)]
mod tests;
