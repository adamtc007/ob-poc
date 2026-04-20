//! ob-poc impl of [`dsl_runtime::service_traits::SemOsContextResolver`].
//!
//! Bridges the plane-crossing trait to
//! `crate::sem_reg::agent::mcp_tools::build_sem_os_service`, which
//! returns an `Arc<dyn sem_os_core::service::CoreService>` whose
//! concrete impl lives in the in-process registry. We then call
//! `.resolve_context(...)` on that service.

use anyhow::Result;
use async_trait::async_trait;
use sem_os_core::context_resolution::{ContextResolutionRequest, ContextResolutionResponse};
use sem_os_core::principal::Principal;
use sqlx::PgPool;

use dsl_runtime::service_traits::SemOsContextResolver;

use crate::sem_reg::agent::mcp_tools::build_sem_os_service;

pub struct ObPocSemOsContextResolver {
    pool: PgPool,
}

impl ObPocSemOsContextResolver {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SemOsContextResolver for ObPocSemOsContextResolver {
    async fn resolve_context(
        &self,
        principal: &Principal,
        request: ContextResolutionRequest,
    ) -> Result<ContextResolutionResponse> {
        let service = build_sem_os_service(&self.pool);
        service
            .resolve_context(principal, request)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}
