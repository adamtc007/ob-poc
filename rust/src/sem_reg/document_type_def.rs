//! Document type definition body — re-export of the canonical
//! definition in `sem_os_core::document_type_def`.
//!
//! Audit-expansion follow-up (2026-05-13): the local types here
//! were byte-identical to `sem_os_core::document_type_def`.
//! Collapsing to a re-export removes 1 entry from the schema-
//! authority drift baseline and keeps `sem_os_core` as the
//! single schema authority per V&S §O7 / ADN §7.3.

pub use sem_os_core::document_type_def::*;
