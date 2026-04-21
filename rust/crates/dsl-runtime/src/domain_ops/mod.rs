//! Domain-plugin ops owned by the data plane.
//!
//! Populated incrementally by Phase 4 of the three-plane architecture refactor.
//! Each submodule contains a set of `CustomOperation` impls decorated with
//! `#[register_custom_op]`. Registration flows through the `inventory`
//! collection in [`crate::custom_op::CustomOperationRegistry::new`] — moving
//! a file here changes nothing about how its ops are discovered at startup.
//!
//! Ops in this module must not reach into `ob-poc` internals. Platform-coupled
//! plugin ops stay in `ob-poc::domain_ops` behind service traits until Phase 5.

pub mod helpers;

pub mod access_review_ops;
pub mod agent_ops;
// Phase 5c-migrate Phase B slice #5 (2026-04-21): attribute_ops relocated
// to `sem_os_postgres::ops::attribute::*`.
pub mod batch_control_ops;
pub mod affinity_graph_cache;
pub mod affinity_ops;
pub mod billing_ops;
pub mod board_ops;
pub mod bods_ops;
pub mod coverage_compute_ops;
pub mod capital_ops;
pub mod cbu_ops;
pub mod cbu_role_ops;
pub mod client_group_ops;
// Phase 5c-migrate Phase B slice #3 (2026-04-21): constellation_ops
// relocated to `sem_os_postgres::ops::constellation::*` as YAML-first
// re-implementations; legacy file deleted.
pub mod control_compute_ops;
pub mod control_ops;
pub mod custody;
pub mod deal_ops;
pub mod dilution_ops;
pub mod discovery_ops;
pub mod docs_bundle_ops;
pub mod document_ops;
pub mod economic_exposure_ops;
pub mod edge_ops;
pub mod entity_ops;
pub mod entity_query;
pub mod evidence_ops;
pub mod graph_validate_ops;
pub mod import_run_ops;
pub mod investor_ops;
pub mod investor_role_ops;
pub mod kyc_case_ops;
pub mod lifecycle_ops;
pub mod manco_ops;
pub mod matrix_overlay_ops;
// Phase 5c-migrate Phase B slice #2 (2026-04-21): navigation_ops relocated
// to `sem_os_postgres::ops::nav::*` as YAML-first re-implementations; legacy
// file deleted.
pub mod observation_ops;
pub mod outreach_ops;
pub mod outreach_plan_ops;
pub mod ownership_ops;
// Phase 5c-migrate Phase B slice #1 (2026-04-21): pack_ops relocated to
// `sem_os_postgres::ops::{pack_select,pack_answer}` as YAML-first
// re-implementations; legacy file deleted.
pub mod partnership_ops;
// Phase 5c-migrate Phase B slice #4 (2026-04-21): phrase_ops relocated to
// `sem_os_postgres::ops::phrase::*` as YAML-first re-implementations;
// legacy file deleted.
pub mod refdata_loader;
pub mod refdata_ops;
pub mod regulatory_ops;
pub mod remediation_ops;
pub mod requirement_ops;
pub mod resource_ops;
pub mod screening_ops;
pub mod research_normalize_ops;
pub mod research_workflow_ops;
// Phase 5c-migrate Phase B slice #6: sem_os_audit_ops → `sem_os_postgres::ops::audit`.
// Phase 5c-migrate Phase B slice #6: sem_os_changeset_ops → `sem_os_postgres::ops::changeset`.
// Phase 5c-migrate Phase B slice #6: sem_os_focus_ops → `sem_os_postgres::ops::focus`.
// Phase 5c-migrate Phase B slice #6: sem_os_governance_ops → `sem_os_postgres::ops::governance`.
// Phase 5c-migrate Phase B slice #7: sem_os_maintenance_ops → `sem_os_postgres::ops::maintenance`.
// Phase 5c-migrate Phase B slice #6: sem_os_registry_ops → `sem_os_postgres::ops::registry_ops`.
pub mod sem_os_schema_ops;
pub mod semantic_ops;
// Phase 5c-migrate Phase B slice #5 (2026-04-21): service_pipeline_ops
// relocated to `sem_os_postgres::ops::service_pipeline::*`.
// Phase 5c-migrate Phase B slice #5 (2026-04-21): session_ops relocated to
// `sem_os_postgres::ops::session::*`.
pub mod shared_atom_ops;
pub mod skeleton_build_ops;
pub mod state_ops;
pub mod team_ops;
pub mod temporal_ops;
pub mod tollgate_evaluate_ops;
pub mod tollgate_ops;
pub mod trading_matrix;
pub mod trading_profile_ca_ops;
pub mod trust_ops;
// Phase 5c-migrate Phase B slice #5 (2026-04-21): view_ops relocated to
// `sem_os_postgres::ops::view::*`.
pub mod ubo_analysis;
pub mod ubo_compute_ops;
pub mod ubo_graph_ops;
pub mod ubo_registry_ops;
pub mod verify_ops;
