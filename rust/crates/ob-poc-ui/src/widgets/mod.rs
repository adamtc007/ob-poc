//! Reusable Widgets
//!
//! Pure widgets that do NOT have access to AppState.
//! They take data as input and return events via return values (not callbacks).

mod entity_search;

pub use entity_search::{entity_search_popup, EntitySearchResponse};
