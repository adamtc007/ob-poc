//! `dsl_types` — Level 0 substrate types.
//!
//! Pure data with serde. No DB, no SemOS, no app coupling.
//! This crate is the bottom of the dependency graph:
//!
//! ```
//! dsl_types  (this crate — std + serde only)
//!     ↑
//! dsl-lang   (parser, compiler, ops/IR)
//!     ↑
//! sem-os     (frontier, resolver, navigation)
//!     ↑
//! ob-poc / bpmn-lite  (apps)
//! ```
//!
//! ## What lives here
//!
//! - `constellation_map_def` — the authored slot/join/cardinality vocabulary
//!   used by both the DSL compiler (`dsl-lang`) and the SemOS resolver.
//!   Moved here from `sem_os_ontology` so `dsl-lang` can reference these
//!   types without depending on the SemOS layer.

pub mod constellation_map_def;
