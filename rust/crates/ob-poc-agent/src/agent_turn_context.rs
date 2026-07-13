//! `AgentTurnContext` — the agent-tier projection of `ob-poc`'s
//! `OrchestratorContext`.
//!
//! T11.2 Part A (2026-07-13). `OrchestratorContext` (in `ob-poc`) mixes 5
//! capability handles (DB pool, verb searcher, lookup service, policy gate,
//! SemOS client) with pure interpretation data in one struct. This type is
//! the narrow, `Clone`-able projection carrying only the agent-tier data —
//! built once per turn by `OrchestratorContext::agent_turn_context()` (in
//! `ob-poc`, the only place that can construct one, since it reads the
//! source struct's private fields) and handed down to agent-tier functions
//! that live in this crate.
//!
//! `OrchestratorContext` itself never crosses into this crate; this
//! projection is what does. No capability handle is a field here — that
//! absence is compiler-enforced, not a per-file-trace claim.
//!
//! `agent_mode`/`goals`/`stage_focus`/`scope` are carried through as
//! read-only/advisory: plain data, but their legality-determining *use* is
//! CP-tier per the design law already ratified for `LegalityGrant` in
//! `ob-poc`. Consumers in this crate may read them for non-legality
//! interpretation (e.g. routing), never to recompute a verdict.
//!
//! See `docs/todo/control-plane/
//! EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md`.

use crate::sage::{RecentIntent, SageEngine};
use crate::semtaxonomy_v2::IntentCompiler;
use sem_os_policy::abac::ActorContext;
use sem_os_types::agent_mode::AgentMode;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Where the utterance originated. Mirrors `ob_poc::agent::orchestrator::
/// UtteranceSource` field-for-field (duplicated rather than shared: it's a
/// 3-variant enum with no behavior, and sharing it would mean this crate
/// depending back on `ob-poc` for a marker type).
#[derive(Debug, Clone)]
pub enum UtteranceSource {
    Chat,
    Mcp,
    Repl,
}

/// Scope context for entity resolution. Mirrors `ob_poc::mcp::
/// scope_resolution::ScopeContext` field-for-field — plain data (see
/// T11.2 Part A's field census), duplicated rather than shared for the
/// same reason as `UtteranceSource`: `mcp/` stays in `ob-poc` (T11.1a
/// ratified answer 2, T12 scope), so there's no shared crate home for
/// this type yet.
#[derive(Debug, Clone, Default)]
pub struct ScopeContext {
    pub client_group_id: Option<Uuid>,
    pub client_group_name: Option<String>,
    pub persona: Option<String>,
}

// Several fields below are unread by today's first two consumers
// (`run_sage_stage`/`run_coder_stage`) but are part of the full agent-tier
// surface `OrchestratorContext` carries — kept for the next functions this
// projection is retrofitted onto, not speculative. `#[allow(dead_code)]`
// rather than trimming, since trimming now would just mean re-adding them
// field-by-field as each future consumer needs one.
#[derive(Clone)]
pub struct AgentTurnContext {
    #[allow(dead_code)]
    pub actor: ActorContext,
    pub session_id: Option<Uuid>,
    #[allow(dead_code)]
    pub case_id: Option<Uuid>,
    pub dominant_entity_id: Option<Uuid>,
    #[allow(dead_code)]
    pub source: UtteranceSource,
    pub sage_engine: Option<Arc<dyn SageEngine>>,
    pub nlci_compiler: Option<Arc<dyn IntentCompiler>>,
    pub pre_sage_entity_kind: Option<String>,
    pub pre_sage_entity_name: Option<String>,
    #[allow(dead_code)]
    pub pre_sage_entity_confidence: Option<f64>,
    pub recent_sage_intents: Vec<RecentIntent>,
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
