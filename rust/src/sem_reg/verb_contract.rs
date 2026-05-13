//! Verb contract body — re-export of the canonical definition in
//! `sem_os_core::verb_contract`.
//!
//! Phase 4.7 audit follow-up (2026-05-13): the local parallel
//! definition that lived here was a strict subset of
//! `sem_os_core::verb_contract::*` (missing 4 footprint fields on
//! `VerbContractBody`, 13 SQL helper fields on `VerbCrudMapping`,
//! `maps_to` on `VerbArgDef`, plus the `VerbOutput` struct
//! entirely). Collapsing to a re-export removes 6 entries from
//! the schema-authority drift allowlist
//! (`VerbContractBody`, `VerbCrudMapping`, `VerbArgDef`,
//! `VerbReturnSpec`, `VerbPrecondition`, `VerbProducesSpec`) and
//! keeps `sem_os_core` as the single schema authority per
//! V&S §O7 / ADN §7.3.
//!
//! All existing import paths through `crate::sem_reg::verb_contract::*`
//! continue to work; struct-literal callers in `src/sem_reg/*`,
//! `src/sem_os_runtime/`, and the integration tests were updated
//! in the same commit to list the additional fields (all default
//! to `None` / `Vec::new()` so behaviour matches the pre-collapse
//! state).

pub use sem_os_core::verb_contract::*;
