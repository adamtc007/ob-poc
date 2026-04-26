//! ob-poc impl of [`dsl_runtime::service_traits::AttributeIdentityService`].
//!
//! Bridges the plane-crossing trait (defined in `dsl-runtime`) to the
//! existing in-crate
//! [`crate::services::attribute_identity_service::AttributeIdentityService`],
//! which runs the multi-namespace UNION query against the operational
//! database.
//!
//! The bridge constructs the in-crate service per-call (matching the
//! pattern already used by every consumer site — the service holds only
//! a `PgPool` and is `Clone`, so this is cheap). The trait surface
//! starts narrow (`resolve_runtime_uuid` only) — see the trait
//! docstring for the slice-by-slice extension plan.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::service_traits::AttributeIdentityService;

use crate::services::attribute_identity_service::AttributeIdentityService as InternalService;

pub struct ObPocAttributeIdentityService {
    pool: PgPool,
}

impl ObPocAttributeIdentityService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AttributeIdentityService for ObPocAttributeIdentityService {
    async fn resolve_runtime_uuid(&self, reference: &str) -> Result<Option<Uuid>> {
        InternalService::new(self.pool.clone())
            .resolve_runtime_uuid(reference)
            .await
    }
}
