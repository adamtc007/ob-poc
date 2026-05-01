//! Frontier disclosure types for resolved SemOS DAG instances.

mod hydrator;

pub use hydrator::{hydrate_frontier, FrontierFact, FrontierFacts, HydrateFrontierError};

/// Reference to one entity instance whose frontier should be disclosed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    pub slot_id: String,
    pub entity_id: String,
    pub current_state: String,
    pub facts: FrontierFacts,
}

/// Frontier for a single resolved entity instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceFrontier {
    pub entity_ref: EntityRef,
    pub current_state: String,
    pub reachable: Vec<ReachableDestination>,
}

/// One destination state reachable from the current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachableDestination {
    pub destination_state: String,
    pub via_verb: Option<String>,
    pub status: GreenWhenStatus,
}

/// Evaluation status for a destination state's postcondition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GreenWhenStatus {
    Green,
    Red {
        missing: Vec<MissingFact>,
        invalid: Vec<InvalidFact>,
    },
    AwaitingCompleteness(CompletenessAssertionStatus),
    Discretionary(DiscretionaryReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingFact {
    pub entity: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidFact {
    pub entity: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletenessAssertionStatus {
    pub assertion: String,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscretionaryReason {
    pub reason: String,
}
