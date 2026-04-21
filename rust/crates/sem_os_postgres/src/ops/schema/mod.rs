//! Schema domain verbs (13 verbs) — SemOS-side YAML-first
//! re-implementation of `rust/config/verbs/sem-reg/schema.yaml`.
//!
//! Three shapes share this module:
//!
//! 1. **Structure semantics** (5 verbs) — project ontology + sem_reg
//!    snapshots + runtime verb registry through
//!    [`SchemaIntrospectionAccess`] (service trait). Single
//!    `describe_entity` helper computes the merged record; each op
//!    is a thin projection.
//! 2. **Introspection / extraction** (5 verbs) — delegate to the
//!    `db_introspect` MCP tool via [`StewardshipDispatch`]. Macro
//!    `introspect_op!` mints each unit struct.
//! 3. **Diagram generation** (3 verbs) — read physical schema via
//!    `information_schema`, load the `AffinityGraph` cache (reused
//!    from `dsl-runtime::domain_ops::affinity_graph_cache`, as
//!    slice #9 does), render Mermaid via `sem_os_core::diagram`.
//!
//! Both AgentModes (Research + Governed) are read-only here.

pub mod diagrams;
pub mod introspect;
pub mod structure;

pub use diagrams::{SchemaGenerateDiscoveryMap, SchemaGenerateErd, SchemaGenerateVerbFlow};
pub use introspect::{
    SchemaCrossReference, SchemaExtractAttributes, SchemaExtractEntities, SchemaExtractVerbs,
    SchemaIntrospect,
};
pub use structure::{
    SchemaDomainDescribe, SchemaEntityDescribe, SchemaEntityListFields,
    SchemaEntityListRelationships, SchemaEntityListVerbs,
};
