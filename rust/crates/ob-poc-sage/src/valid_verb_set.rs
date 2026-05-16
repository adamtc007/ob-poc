//! Valid-verb-set DTO surface.
//!
//! Pure data types that describe "the set of verbs legal in this
//! session context, right now." The computation that produces a
//! `ValidVerbSet` reaches `sem_os_core` constellation maps + the
//! constellation runtime and stays in ob-poc as the
//! `ValidVerbSetEngine` implementation. These types live here so the
//! trait surface (in `crate::engine`) and any external consumer
//! (the agent) can reference them without depending on ob-poc.
//!
//! Phase 2.2 of the Sage ACP capability plan.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A verb that is legal in the current session context.
#[derive(Debug, Clone)]
pub struct VerbCandidate {
    pub verb_fqn: String,
    pub entity_id: Option<Uuid>,
    pub entity_type: String,
    pub source: VerbSource,
    pub priority: u32,
    pub keywords: Vec<String>,
}

/// How this verb became part of the valid set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbSource {
    /// Outgoing FSM transition from current entity state.
    FsmTransition,
    /// Creation verb for an entity that doesn't exist yet.
    CreationVerb,
    /// Always available (observation verbs: read, list, show).
    AlwaysAvailable,
}

/// The computed set of valid verbs for a session context.
#[derive(Debug, Clone)]
pub struct ValidVerbSet {
    pub verbs: Vec<VerbCandidate>,
    pub client_group_id: Uuid,
    pub constellation_id: String,
    pub computed_at: DateTime<Utc>,
}

impl ValidVerbSet {
    /// Get all verb FQNs in the set.
    pub fn verb_fqns(&self) -> Vec<&str> {
        self.verbs.iter().map(|v| v.verb_fqn.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if a specific verb FQN is in the valid set.
    pub fn contains_verb(&self, fqn: &str) -> bool {
        self.verbs.iter().any(|v| v.verb_fqn == fqn)
    }

    /// Convert to a HashSet for passing to the constrained embedding search.
    pub fn to_allowed_set(&self) -> HashSet<String> {
        self.verbs.iter().map(|v| v.verb_fqn.clone()).collect()
    }
}
