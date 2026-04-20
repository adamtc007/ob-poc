//! ob-poc impl of [`dsl_runtime::service_traits::ConstellationRuntime`].
//!
//! Bridges the plane-crossing `ConstellationRuntime` trait (defined in
//! `dsl-runtime`) to the in-crate
//! [`crate::sem_os_runtime::constellation_runtime`] module, which
//! resolves built-in constellation maps and walks them against
//! persisted CBU / case state. Both `handle_constellation_*` functions
//! return ob-poc-internal types (`HydratedConstellation`,
//! `ConstellationSummary`); the bridge projects each through
//! `serde_json::to_value` so the trait surface stays JSON-shaped.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::service_traits::ConstellationRuntime;

use crate::sem_os_runtime::constellation_runtime::{
    handle_constellation_hydrate, handle_constellation_summary,
};

pub struct ObPocConstellationRuntime {
    pool: PgPool,
}

impl ObPocConstellationRuntime {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConstellationRuntime for ObPocConstellationRuntime {
    async fn hydrate(
        &self,
        cbu_id: Uuid,
        case_id: Option<Uuid>,
        map_name: &str,
    ) -> Result<serde_json::Value> {
        let result = handle_constellation_hydrate(&self.pool, cbu_id, case_id, map_name).await?;
        Ok(serde_json::to_value(result)?)
    }

    async fn summary(
        &self,
        cbu_id: Uuid,
        case_id: Option<Uuid>,
        map_name: &str,
    ) -> Result<serde_json::Value> {
        let result = handle_constellation_summary(&self.pool, cbu_id, case_id, map_name).await?;
        Ok(serde_json::to_value(result)?)
    }
}
