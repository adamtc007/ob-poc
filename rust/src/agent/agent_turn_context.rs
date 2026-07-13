//! `AgentTurnContext` — the agent-tier projection of `OrchestratorContext`.
//!
//! T11.2 Part A (2026-07-13): `OrchestratorContext` mixes 5 capability
//! handles (`pool`, `verb_searcher`, `lookup_service`, `policy_gate`,
//! `sem_os_client`) with pure interpretation data in one struct, so every
//! candidate agent-tier function (`run_sage_stage`, `run_coder_stage`, ...)
//! that took `&OrchestratorContext` wholesale was structurally blocked from
//! moving to `ob-poc-agent` — doing so would hand agent-tier code the
//! capability handles directly (an L1 violation).
//!
//! `OrchestratorContext` itself is NOT restructured — it has 4 external
//! construction sites (`sequencer.rs`, `agent/harness/stub.rs`,
//! `api/agent_service.rs`, plus its own tests) and is legitimately
//! CP-tier-resident. `AgentTurnContext` is a derived, `Clone`-able
//! projection instead — same pattern as `LegalityGrant` for the legality
//! verdict. No capability-handle field ever appears here; that absence is
//! the grep-provable enforcement mechanism.
//!
//! `agent_mode`/`goals`/`stage_focus`/`scope` are carried through as
//! read-only/advisory — plain data, but their legality-determining *use* is
//! CP-tier per the design law already ratified for `LegalityGrant`. Agent-
//! tier code may read them for non-legality interpretation (e.g. routing),
//! never to recompute a verdict.
//!
//! See `docs/todo/control-plane/
//! EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md`.

use crate::agent::orchestrator::{OrchestratorContext, UtteranceSource};
use crate::mcp::scope_resolution::ScopeContext;
use crate::sem_reg::abac::ActorContext;
use sem_os_types::agent_mode::AgentMode;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// Several fields below are unread by today's first two consumers
// (`run_sage_stage`/`run_coder_stage`) but are part of the full agent-tier
// surface `OrchestratorContext` carries — kept for the next functions this
// projection is retrofitted onto, not speculative. `#[allow(dead_code)]`
// rather than trimming, since trimming now would just mean re-adding them
// field-by-field as each future consumer needs one.
#[derive(Clone)]
pub(crate) struct AgentTurnContext {
    #[allow(dead_code)]
    pub actor: ActorContext,
    pub session_id: Option<Uuid>,
    #[allow(dead_code)]
    pub case_id: Option<Uuid>,
    pub dominant_entity_id: Option<Uuid>,
    #[allow(dead_code)]
    pub source: UtteranceSource,
    pub sage_engine: Option<Arc<dyn crate::sage::SageEngine>>,
    pub nlci_compiler: Option<Arc<dyn crate::semtaxonomy_v2::IntentCompiler>>,
    pub pre_sage_entity_kind: Option<String>,
    pub pre_sage_entity_name: Option<String>,
    #[allow(dead_code)]
    pub pre_sage_entity_confidence: Option<f64>,
    pub recent_sage_intents: Vec<crate::sage::RecentIntent>,
    #[allow(dead_code)]
    pub discovery_selected_domain: Option<String>,
    #[allow(dead_code)]
    pub discovery_selected_family: Option<String>,
    #[allow(dead_code)]
    pub discovery_selected_constellation: Option<String>,
    #[allow(dead_code)]
    pub discovery_answers: HashMap<String, String>,
    #[allow(dead_code)]
    pub session_cbu_ids: Option<Vec<Uuid>>,
    // CP-authoritative, advisory-only — see module doc.
    #[allow(dead_code)]
    pub agent_mode: AgentMode,
    pub goals: Vec<String>,
    pub stage_focus: Option<String>,
    #[allow(dead_code)]
    pub scope: Option<ScopeContext>,
}

impl OrchestratorContext {
    /// Project the agent-tier-relevant fields out of `self`. No capability
    /// handle is ever included — see the module doc for why that's load-
    /// bearing, not incidental.
    pub(crate) fn agent_turn_context(&self) -> AgentTurnContext {
        AgentTurnContext {
            actor: self.actor.clone(),
            session_id: self.session_id,
            case_id: self.case_id,
            dominant_entity_id: self.dominant_entity_id,
            source: self.source.clone(),
            sage_engine: self.sage_engine.clone(),
            nlci_compiler: self.nlci_compiler.clone(),
            pre_sage_entity_kind: self.pre_sage_entity_kind.clone(),
            pre_sage_entity_name: self.pre_sage_entity_name.clone(),
            pre_sage_entity_confidence: self.pre_sage_entity_confidence,
            recent_sage_intents: self.recent_sage_intents.clone(),
            discovery_selected_domain: self.discovery_selected_domain.clone(),
            discovery_selected_family: self.discovery_selected_family.clone(),
            discovery_selected_constellation: self.discovery_selected_constellation.clone(),
            discovery_answers: self.discovery_answers.clone(),
            session_cbu_ids: self.session_cbu_ids.clone(),
            agent_mode: self.agent_mode,
            goals: self.goals.clone(),
            stage_focus: self.stage_focus.clone(),
            scope: self.scope.clone(),
        }
    }
}
