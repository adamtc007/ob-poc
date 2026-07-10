//! Service Resources Pipeline
//!
//! This module implements the CBU Service → Resource Discovery → Unified Dictionary → Provisioning pipeline.
//!
//! ## Overview
//!
//! ```text
//! ServiceIntent (what CBU wants)
//!        ↓
//! ResourceDiscoveryEngine (derive required SRDEFs)
//!        ↓
//! SrdefDiscoveryReason (audit trail)
//!        ↓
//! AttributeRollupEngine (build unified attr requirements)
//!        ↓
//! CbuUnifiedAttrRequirements (per-CBU attr needs)
//!        ↓
//! PopulationEngine (fill values from sources and canonical derived plane)
//!        ↓
//! Effective CBU values (legacy non-derived rows + canonical derived projection)
//!        ↓
//! ProvisioningOrchestrator (when attrs satisfied)
//!        ↓
//! ProvisioningRequest → ProvisioningEvent
//!        ↓
//! ReadinessEngine (compute "good to transact")
//!        ↓
//! CbuServiceReadiness
//! ```
//!
//! ## Key Tables
//!
//! | Table | Purpose |
//! |-------|---------|
//! | `service_intents` | What CBU wants |
//! | `srdef_discovery_reasons` | Why SRDEFs were discovered |
//! | `cbu_unified_attr_requirements` | Rolled-up attr requirements |
//! | `cbu_attr_values` | Legacy/manual/non-derived CBU values |
//! | `derived_attribute_values` | Canonical derived value history |
//! | `v_cbu_derived_values` | CBU projection of canonical derived values |
//! | `provisioning_requests` | Append-only request log |
//! | `provisioning_events` | Append-only event log |
//! | `cbu_service_readiness` | Derived readiness status |

pub mod discovery;
pub mod onboarding_data_request;
pub mod provisioning;

pub mod service;
pub mod srdef_loader;
pub mod types;

// Re-export main types
pub use types::{AttributeSource, EvidenceRef, NewServiceIntent, SetCbuAttrValue};

pub use service::ServiceResourcePipelineService;

pub use onboarding_data_request::OnboardingDataRequestService;

// Re-export loader types
pub use srdef_loader::{load_and_sync_srdefs, load_srdefs_from_config};

// Re-export discovery engine types
pub use discovery::{
    run_discovery_pipeline, run_discovery_pipeline_in, AttributeRollupEngine, PopulationEngine,
};

// Re-export provisioning types
pub use provisioning::{run_provisioning_pipeline, ReadinessEngine};
