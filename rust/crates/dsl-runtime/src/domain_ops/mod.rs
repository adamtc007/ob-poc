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
pub mod batch_control_ops;
pub mod bods_ops;
pub mod coverage_compute_ops;
pub mod custody;
pub mod docs_bundle_ops;
pub mod document_ops;
pub mod economic_exposure_ops;
pub mod entity_ops;
pub mod graph_validate_ops;
pub mod import_run_ops;
pub mod matrix_overlay_ops;
pub mod outreach_plan_ops;
pub mod pack_ops;
pub mod research_normalize_ops;
pub mod sem_os_changeset_ops;
pub mod sem_os_focus_ops;
pub mod sem_os_governance_ops;
pub mod semantic_ops;
pub mod state_ops;
pub mod temporal_ops;
pub mod tollgate_evaluate_ops;
pub mod tollgate_ops;
pub mod trading_matrix;
pub mod verify_ops;
