//! Engine traits that the Sage ACP runtime consumes.
//!
//! Phase 2.2 of the Sage ACP capability plan (D-engines: trait
//! abstraction). The traits live here in `ob-poc-sage`; the concrete
//! impls live in `ob-poc` (so they can reach `sem_os_runtime`,
//! `database`, `agent::learning`, `mcp::verb_search`); the binary
//! integrator wires the impl into the agent at startup. This is what
//! lets `ob-poc-agent` reach Sage's app-side engines without
//! depending on `ob-poc`.
//!
//! See locked decision 6 of the capability-crate restructure plan
//! (`docs/todo/capability-crate-restructure-v1.md` §6).
//!
//! ## Traits
//!
//! - [`SageEngine`] — classifies a raw utterance + session context
//!   into an [`OutcomeIntent`]. Both the deterministic classifier and
//!   the LLM-driven classifier implement this trait.
//! - [`ValidVerbSetEngine`] — given a workspace / constellation /
//!   entity-state snapshot, returns the deterministic set of verbs
//!   that are legal right now. Used to constrain LLM output to
//!   sanctioned primitives.

use anyhow::Result;
use async_trait::async_trait;

use crate::context::SageContext;
use crate::outcome::OutcomeIntent;

// `EntityState` and `ValidVerbSet` are surfaced only when the
// `database` feature is on (session_context lives there). The
// SageEngine trait below is unconditional; ValidVerbSetEngine and
// ValidVerbSetScope are gated to match.
#[cfg(feature = "database")]
use crate::session_context::EntityState;
#[cfg(feature = "database")]
use crate::valid_verb_set::ValidVerbSet;
#[cfg(feature = "database")]
use uuid::Uuid;

/// Sage classifier surface.
///
/// Classifies user intent from raw utterance + session context.
///
/// ## Contract
/// - Never receives verb FQNs (E-SAGE-2)
/// - Always receives raw utterance text (not entity-resolved text)
///   (E-SAGE-1)
/// - Always returns a valid [`OutcomeIntent`] (degrades to Low
///   confidence stub on failure)
#[async_trait]
pub trait SageEngine: Send + Sync {
    async fn classify(&self, utterance: &str, context: &SageContext) -> Result<OutcomeIntent>;
}

/// Identifies a workspace + constellation stack + entity-state
/// snapshot for which a valid verb set should be computed.
///
/// The runtime resolves the constellation stack from
/// `(workspace, constellation_id)` against the SemOS seed corpus and
/// composes the legal verb set across the stack.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct ValidVerbSetScope<'a> {
    /// Client group this session is scoped to.
    pub client_group_id: Uuid,
    /// Workspace label (e.g. "cbu", "kyc", "deal").
    pub workspace: &'a str,
    /// Session-facing constellation identifier
    /// (e.g. "struct.lux.ucits.sicav", "group.ownership").
    pub constellation_id: &'a str,
    /// Current entity-state snapshot for the session.
    pub entity_states: &'a [EntityState],
}

/// Computes the deterministic valid verb set for a session scope.
///
/// Used by the Sage ACP runtime to constrain LLM output to
/// sanctioned primitives — the LLM selects from this set rather than
/// emitting free-text DSL.
#[cfg(feature = "database")]
#[async_trait]
pub trait ValidVerbSetEngine: Send + Sync {
    /// Compute the valid verb set for the given scope.
    async fn compute(&self, scope: ValidVerbSetScope<'_>) -> Result<ValidVerbSet>;
}
