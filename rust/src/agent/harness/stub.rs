//! Stub pipeline for deterministic scenario testing.
//!
//! Uses `HybridVerbSearcher::minimal()` which returns NoMatch for everything,
//! making it ideal for testing governance, policy gate, and trace accuracy
//! scenarios that don't depend on verb discovery.
//!
//! For scenarios that need specific verb matches, use live mode with a real DB.

use std::sync::Arc;
use uuid::Uuid;

use crate::agent::orchestrator::{OrchestratorContext, UtteranceSource};
use crate::mcp::verb_search::HybridVerbSearcher;
use crate::policy::PolicyGate;
use crate::sem_reg::abac::ActorContext;

use super::{ModeExpectations, SessionSeed};

/// Build an OrchestratorContext for stub mode testing.
///
/// Uses minimal verb searcher (no DB, no embedder) and configurable PolicyGate.
#[cfg(feature = "database")]
pub fn build_stub_context(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    seed: &SessionSeed,
    mode: &ModeExpectations,
) -> OrchestratorContext {
    let actor = ActorContext {
        actor_id: seed.actor.actor_id.clone(),
        roles: seed.actor.roles.clone(),
        department: None,
        clearance: None,
        jurisdictions: vec![],
    };

    let policy = PolicyGate {
        strict_single_pipeline: mode.strict_single_pipeline,
        allow_raw_execute: mode.allow_raw_execute,
        allow_direct_dsl: mode.allow_direct_dsl,
        strict_semreg: mode.strict_semreg,
        allow_legacy_generate: false,
    };

    OrchestratorContext {
        actor,
        session_id: Some(session_id),
        case_id: Some(session_id),
        dominant_entity_id: None,
        scope: None,
        pool: pool.clone(),
        verb_searcher: Arc::new(HybridVerbSearcher::minimal()),
        lookup_service: None,
        policy_gate: Arc::new(policy),
        source: UtteranceSource::Chat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_context_uses_strict_defaults() {
        let mode = ModeExpectations::default();
        assert!(mode.strict_semreg);
        assert!(mode.strict_single_pipeline);
        assert!(!mode.allow_direct_dsl);
    }

    #[test]
    fn test_actor_seed_defaults() {
        let seed = SessionSeed::default();
        assert_eq!(seed.actor.actor_id, "test.user");
        assert_eq!(seed.actor.roles, vec!["viewer"]);
    }
}
