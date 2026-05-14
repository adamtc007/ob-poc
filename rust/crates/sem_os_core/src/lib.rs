pub mod abac;
pub mod acp_projection;
pub mod affinity;
pub mod authoring;
pub(crate) mod constellation_family_def;
pub mod constellation_map_def;
pub mod context_policy;
pub mod context_resolution;
pub mod diagram;
pub mod domain_pack;
pub mod enforce;
pub mod error;
pub mod execution;
pub(crate) mod gates;
pub mod grounding;
pub mod ids;
pub(crate) mod macro_def;
pub mod observatory;
pub mod ports;
pub mod principal;
pub mod proto;
pub(crate) mod security;
pub mod seeds;
pub mod service;
pub mod state_simulation;
pub mod stewardship;
pub mod types;

// ── Body type modules ─────────────────────────────────────────
pub mod attribute_def;
pub mod derivation;
pub mod derivation_spec;
pub mod document_type_def;
pub mod entity_type_def;
pub mod evidence;
pub mod evidence_strategy_def;
pub mod membership;
pub mod proof_obligation_def;
pub mod service_resource_def;
pub mod state_graph_def;

// sem_os_core-split v1 Phase 2 (2026-05-14): pure-type ontology leaves
// relocated to `sem_os_ontology`. Compat re-exports keep
// `sem_os_core::<def>::*` paths resolving for all downstream consumers.
// Removed in Phase 12.
pub use sem_os_ontology::observation_def;
pub use sem_os_ontology::policy_rule;
pub use sem_os_ontology::relationship_type_def;
pub use sem_os_ontology::requirement_profile_def;
pub use sem_os_ontology::state_machine_def;
pub use sem_os_ontology::taxonomy_def;
pub use sem_os_ontology::universe_def;
pub use sem_os_ontology::verb_contract;
pub use sem_os_ontology::view_def;
