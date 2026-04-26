//! Semantic onboarding stages + derived state (ob-poc ontology + DB-backed).
//!
//! Unifies the two capabilities `semantic.*` plugin ops need:
//!
//! - **Derived state**: given a CBU id, compute which stages are complete,
//!   in progress, or blocked based on the current entity inventory.
//! - **Stage catalogue**: the ontology's static stage definitions (ordered,
//!   product-filtered, lookup by code).
//!
//! Both sit on a single trait because one impl — ob-poc's
//! `SemanticStageRegistry` loaded from `config/ontology/semantic_stage_map.yaml`
//! at startup, plus a `PgPool` clone — serves both needs.
//!
//! Moved under trait abstraction in Phase 5a so the consumer ops (`semantic.*`)
//! can relocate to `dsl-runtime` while the ontology config loader stays in
//! `ob-poc`. Consumers obtain the impl via
//! [`crate::VerbExecutionContext::service::<dyn SemanticStateService>`].

use async_trait::async_trait;
use ob_poc_types::semantic_stage::{SemanticState, StageDefinition};
use uuid::Uuid;

/// Semantic onboarding capability (stage catalogue + DB-backed derivation).
#[async_trait]
pub trait SemanticStateService: Send + Sync {
    /// Derive the current onboarding state for `cbu_id`.
    ///
    /// Returns `Err` if the CBU doesn't exist, the stage map can't be
    /// resolved, or a downstream DB query fails.
    async fn derive(&self, cbu_id: Uuid) -> anyhow::Result<SemanticState>;

    /// All stages in topological order (dependencies first).
    fn list_stages(&self) -> Vec<StageDefinition>;

    /// Stages required when the CBU subscribes to `product` (e.g. `"CUSTODY"`).
    /// Returns stage codes; callers resolve individual definitions via
    /// [`Self::get_stage`].
    fn stages_for_product(&self, product: &str) -> Vec<String>;

    /// Look up one stage definition by its code.
    fn get_stage(&self, code: &str) -> Option<StageDefinition>;
}
