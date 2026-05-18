//! `bpmn-controller` — BPMN pool lifecycle and instance kick-off for OB-POC.
//!
//! This crate is the single authoritative interface between ob-poc and
//! bpmn-lite's infrastructure. It exposes Rust functions (not a gRPC server)
//! that ob-poc-web calls directly. Sage agent DSL verbs (L6) dispatch into
//! these functions via the standard Shape 1 path.
//!
//! ## Modules
//!
//! - `loader`   — pool lifecycle (provision, deprovision, list, status)
//! - `instance` — instance kick-off and read operations (start, status, list)
//! - `k8s`      — K8s client abstraction (L1: placeholder; L3: real kube::Client)
//! - `error`    — `BpmnControllerError` for structured error handling
//!
//! ## Dependency discipline
//!
//! This crate depends on `ob-poc-types` for shared DTOs and on `sqlx` for
//! Postgres access. It does NOT depend on any execution-tier crate (dsl-runtime,
//! sequencer, domain_ops, sem_os_*). It is a leaf in the ob-poc dependency graph.

pub(crate) mod deployment;
pub mod error;
pub mod instance;
pub mod k8s;
pub mod loader;

pub use error::BpmnControllerError;
pub use instance::{instance_status, list_tenant_instances, start_instance};
pub use k8s::K8sClient;
pub use loader::{deprovision_pool, list_pool_tenants, list_pools, pool_status, provision_pool};
