//! SemOS context resolution — the 12-step pipeline that produces a
//! `ContextResolutionResponse` from a `ContextResolutionRequest`.
//!
//! `resolve_context` is the entry point used by `affinity_ops` (and a
//! handful of other governance-aware ops) to evaluate ABAC, security
//! labels, and verb prune reasons against a principal. The full
//! [`sem_os_core::service::CoreService`] trait is much larger (~12
//! methods covering manifest, export, validate, publish, etc.) and
//! its concrete impl lives deep in `crate::sem_reg::agent::mcp_tools`
//! plus `sem_os_postgres`.
//!
//! This trait projects only the `resolve_context` method onto a
//! plane-crossable boundary. Both input and output types
//! (`ContextResolutionRequest`, `ContextResolutionResponse`,
//! `Principal`) are already in `sem_os_core`, so no types-extraction
//! is required.
//!
//! Introduced in Phase 5a composite-blocker #24 for `affinity_ops`.
//! The ob-poc bridge (`ObPocSemOsContextResolver`) delegates to
//! `crate::sem_reg::agent::mcp_tools::build_sem_os_service(pool).resolve_context(...)`.
//! Consumers obtain the impl via
//! [`crate::VerbExecutionContext::service::<dyn SemOsContextResolver>`].

use anyhow::Result;
use async_trait::async_trait;
use sem_os_core::context_resolution::{ContextResolutionRequest, ContextResolutionResponse};
use sem_os_core::principal::Principal;

/// Single-method trait for SemOS context resolution. Returns
/// `anyhow::Result` to keep `sem_os_core::service::SemOsError` out
/// of the boundary; the bridge converts internally.
#[async_trait]
pub trait SemOsContextResolver: Send + Sync {
    async fn resolve_context(
        &self,
        principal: &Principal,
        request: ContextResolutionRequest,
    ) -> Result<ContextResolutionResponse>;
}
