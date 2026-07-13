//! Projection from `OrchestratorContext` to `ob_poc_agent::agent_turn_context::
//! AgentTurnContext`.
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
//! CP-tier-resident. `AgentTurnContext` (defined in `ob-poc-agent`, since
//! it's the type that crosses the crate boundary — `OrchestratorContext`
//! never does) is a derived, `Clone`-able projection instead — same pattern
//! as `LegalityGrant` for the legality verdict. No capability-handle field
//! ever appears in it; that absence is the grep-provable enforcement
//! mechanism.
//!
//! Two fields have no shared type between the crates and are converted
//! field-by-field here: `UtteranceSource` and `ScopeContext` are each
//! duplicated (small, behaviorless data types) in `ob-poc-agent` rather
//! than shared, per that crate's own module doc.
//!
//! See `docs/todo/control-plane/
//! EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md`.

use crate::agent::orchestrator::{OrchestratorContext, UtteranceSource};
use ob_poc_agent::agent_turn_context::{
    AgentTurnContext, ScopeContext as AgentScopeContext, UtteranceSource as AgentUtteranceSource,
};

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
            source: match self.source {
                UtteranceSource::Chat => AgentUtteranceSource::Chat,
                UtteranceSource::Mcp => AgentUtteranceSource::Mcp,
                UtteranceSource::Repl => AgentUtteranceSource::Repl,
            },
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
            scope: self.scope.as_ref().map(|s| AgentScopeContext {
                client_group_id: s.client_group_id,
                client_group_name: s.client_group_name.clone(),
                persona: s.persona.clone(),
            }),
        }
    }
}
