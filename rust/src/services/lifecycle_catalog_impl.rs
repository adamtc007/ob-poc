//! ob-poc impl of [`dsl_runtime::service_traits::LifecycleCatalog`].
//!
//! Bridges the trait to the global taxonomy-loaded `OntologyService`,
//! preserving the fallback semantics the in-crate helpers used to have:
//! missing entity type → transition rejected / state non-terminal /
//! empty next-state set.

use dsl_runtime::service_traits::LifecycleCatalog;

use crate::ontology::{is_terminal_state, is_valid_transition, ontology};

pub struct ObPocLifecycleCatalog;

impl ObPocLifecycleCatalog {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocLifecycleCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleCatalog for ObPocLifecycleCatalog {
    fn is_valid_transition(&self, entity_type: &str, from: &str, to: &str) -> bool {
        match ontology().get_lifecycle(entity_type) {
            Some(lifecycle) => is_valid_transition(lifecycle, from, to),
            None => false,
        }
    }

    fn is_terminal_state(&self, entity_type: &str, state: &str) -> bool {
        match ontology().get_lifecycle(entity_type) {
            Some(lifecycle) => is_terminal_state(lifecycle, state),
            None => false,
        }
    }

    fn valid_next_states(&self, entity_type: &str, state: &str) -> Vec<String> {
        ontology()
            .valid_next_states(entity_type, state)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }
}
