//! Taxonomy definition body — re-export of the canonical
//! definition in `sem_os_core::taxonomy_def`.
//!
//! Audit-expansion follow-up (2026-05-13): the local types here
//! were byte-identical to `sem_os_core::taxonomy_def` (only
//! BTreeMap import style differed). Collapsing to a re-export
//! keeps `sem_os_core` as the single schema authority per V&S §O7
//! / ADN §7.3.

pub use sem_os_core::taxonomy_def::*;
