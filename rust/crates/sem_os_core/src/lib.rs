pub mod abac;
pub mod acp_projection;
pub mod affinity;
pub mod authoring;
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
// derivation remains here until Phase 5 — it reaches
// `crate::security::compute_inherited_label` (a pub(crate) policy-tier
// helper). When security promotes + moves to sem_os_policy, derivation
// can decide between joining ontology or following security to policy.
pub mod derivation;

// sem_os_core-split v1 Phases 2–3 (2026-05-14): ontology modules
// relocated to `sem_os_ontology`. Compat re-exports keep
// `sem_os_core::<def>::*` paths resolving for all downstream consumers.
// The pub(crate) modules (constellation_family_def, constellation_map_def,
// macro_def) are de facto promoted to pub by this move — that promotion
// was a locked ADR decision (see docs/todo/sem-os-core-split-v1.md §5).
// Removed in Phase 12.
pub use sem_os_ontology::attribute_def;
pub use sem_os_ontology::constellation_family_def;
pub use sem_os_ontology::constellation_map_def;
pub use sem_os_ontology::derivation_spec;
pub use sem_os_ontology::document_type_def;
pub use sem_os_ontology::entity_type_def;
pub use sem_os_ontology::evidence;
pub use sem_os_ontology::evidence_strategy_def;
pub use sem_os_ontology::macro_def;
pub use sem_os_ontology::membership;
pub use sem_os_ontology::observation_def;
pub use sem_os_ontology::policy_rule;
pub use sem_os_ontology::proof_obligation_def;
pub use sem_os_ontology::relationship_type_def;
pub use sem_os_ontology::requirement_profile_def;
pub use sem_os_ontology::service_resource_def;
pub use sem_os_ontology::state_graph_def;
pub use sem_os_ontology::state_machine_def;
pub use sem_os_ontology::taxonomy_def;
pub use sem_os_ontology::universe_def;
pub use sem_os_ontology::verb_contract;
pub use sem_os_ontology::view_def;
