//! Helpers for building a [`SageContext`] from REPL session state.

use crate::types::SageContext;

/// Build a [`SageContext`] from a domain name (e.g., the current workspace).
///
/// For v0.2 this is intentionally thin: domain is the only signal
/// wired up.  History and process_name are filled by the caller once
/// those session fields are available.
pub fn context_from_session(domain: Option<&str>) -> SageContext {
    SageContext {
        domain: domain.map(String::from),
        history: vec![],
        process_name: None,
    }
}
