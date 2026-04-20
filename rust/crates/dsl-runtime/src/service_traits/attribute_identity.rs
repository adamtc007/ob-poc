//! Attribute identity resolution across the legacy dictionary, the
//! operational `attribute_registry`, and SemOS-governed attribute
//! definitions (`sem_reg.v_active_attribute_defs`).
//!
//! The concrete service ([`crate::services::ServiceRegistry`] impl via
//! `ObPocAttributeIdentityService`) delegates to the ob-poc-side
//! `crate::services::attribute_identity_service::AttributeIdentityService`,
//! which runs the multi-namespace UNION query against the operational
//! database. Plugin ops in `dsl-runtime` consume this trait via
//! [`crate::VerbExecutionContext::service::<dyn AttributeIdentityService>`].
//!
//! # Surface
//!
//! Slice #8 of Phase 5a relocates `observation_ops`, which only needs
//! the `resolve_runtime_uuid` projection — a single Uuid lookup over an
//! attribute reference (FQN, registry id, dictionary uuid, alias, or
//! display name). The richer `ResolvedAttributeIdentity` projection
//! (description, data_type, source/sink config, group id, etc.) used by
//! `attribute_ops` will be added when that file is lifted in a later
//! slice. Trait additions follow the same recipe — extend the trait,
//! extend the bridge, no new registration.

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// Attribute identity resolver across the currently coexisting
/// namespaces (legacy dictionary, operational registry, SemOS-governed
/// attribute defs).
#[async_trait]
pub trait AttributeIdentityService: Send + Sync {
    /// Resolve `reference` (FQN, registry id, dictionary uuid, alias,
    /// or display name) to its runtime UUID — preferring the registry
    /// uuid over the legacy dictionary uuid when both are present.
    /// Returns `Ok(None)` when no namespace recognises the reference.
    async fn resolve_runtime_uuid(&self, reference: &str) -> Result<Option<Uuid>>;
}
