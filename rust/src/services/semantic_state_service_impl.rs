//! ob-poc implementation of [`dsl_runtime::service_traits::SemanticStateService`].
//!
//! Wraps the existing `database::derive_semantic_state` function with the
//! ontology stage registry (loaded from YAML at startup). Registered into
//! the platform `ServiceRegistry` by `ob-poc-web` on boot so plugin ops
//! that relocated to `dsl-runtime` (currently `semantic.*`) can resolve it.

use std::sync::Arc;

use async_trait::async_trait;
use ob_poc_types::semantic_stage::{SemanticState, StageDefinition};
use sqlx::PgPool;
use uuid::Uuid;

use dsl_runtime::service_traits::SemanticStateService;

use crate::database::derive_semantic_state;
use crate::ontology::SemanticStageRegistry;

/// Production impl backed by the ontology stage registry + platform pool.
pub struct ObPocSemanticStateService {
    pool: PgPool,
    registry: Arc<SemanticStageRegistry>,
}

impl ObPocSemanticStateService {
    /// Construct an impl. Call at host startup — the stage registry is
    /// loaded once from `config/ontology/semantic_stage_map.yaml`.
    pub fn new(pool: PgPool) -> anyhow::Result<Self> {
        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow::anyhow!("failed to load semantic stage map: {}", e))?;
        Ok(Self {
            pool,
            registry: Arc::new(registry),
        })
    }

    /// Construct from a pre-loaded registry (test doubles, custom configs).
    pub fn with_registry(pool: PgPool, registry: Arc<SemanticStageRegistry>) -> Self {
        Self { pool, registry }
    }
}

#[async_trait]
impl SemanticStateService for ObPocSemanticStateService {
    async fn derive(&self, cbu_id: Uuid) -> anyhow::Result<SemanticState> {
        derive_semantic_state(&self.pool, &self.registry, cbu_id)
            .await
            .map_err(|e| anyhow::anyhow!("derive_semantic_state failed: {}", e))
    }

    fn list_stages(&self) -> Vec<StageDefinition> {
        self.registry.stages_in_order().cloned().collect()
    }

    fn stages_for_product(&self, product: &str) -> Vec<String> {
        self.registry
            .stages_for_product(product)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn get_stage(&self, code: &str) -> Option<StageDefinition> {
        self.registry.get_stage(code).cloned()
    }
}
