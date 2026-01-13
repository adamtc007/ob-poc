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
//! PopulationEngine (fill values from sources)
//!        ↓
//! CbuAttrValues (populated values)
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
//! | `cbu_attr_values` | Populated attribute values |
//! | `provisioning_requests` | Append-only request log |
//! | `provisioning_events` | Append-only event log |
//! | `cbu_service_readiness` | Derived readiness status |

pub mod discovery;
pub mod provisioning;
pub mod service;
pub mod srdef_loader;
pub mod types;

// Re-export main types
pub use types::{
    AttributeConflict,
    // Report types
    AttributeGapReport,
    AttributeSource,
    BlockingReason,
    BlockingReasonType,

    CbuAttrValue,
    // Readiness types
    CbuServiceReadiness,
    // Attribute types
    CbuUnifiedAttrRequirement,
    DiscoveredSrdef,
    DiscoveryResponse,
    EventDirection,
    EventKind,
    EvidenceRef,
    ExplainRef,

    MissingAttribute,
    NewProvisioningRequest,
    NewServiceIntent,
    NewSrdefDiscovery,

    OwnerProvisioningResult,
    ProvisioningEvent,
    ProvisioningPayload,
    // Provisioning types
    ProvisioningRequest,
    ProvisioningStatus,
    ProvisioningStrategy,
    ReadinessResponse,
    ReadinessStatus,
    ReadinessSummary,
    RequestedBy,
    RequirementStrength,
    ResultExplanation,

    // Core types
    ServiceIntent,
    // API response types
    ServiceIntentResponse,
    ServiceIntentStatus,

    ServiceReadinessDetail,
    SetCbuAttrValue,
    // SRDEF types
    Srdef,
    SrdefDiscoveryReason,
    SrdefReadinessCheck,
};

pub use service::ServiceResourcePipelineService;

// Re-export loader types
pub use srdef_loader::{
    load_and_sync_srdefs, load_srdefs_from_config, LoadedSrdef, LoadedSrdefAttribute,
    SrdefConfigFile, SrdefLoader, SrdefRegistry, SyncResult,
};

// Re-export discovery engine types
pub use discovery::{
    run_discovery_pipeline, AttributeRollupEngine, DiscoveredSrdefInfo, DiscoveryResult,
    PipelineResult, PopulationEngine, PopulationResult, ResourceDiscoveryEngine, RollupResult,
};

// Re-export provisioning types
pub use provisioning::{
    run_provisioning_pipeline, FullPipelineResult, ProvisionResult, ProvisioningOrchestrator,
    ProvisioningOrchestratorResult, ReadinessComputeResult, ReadinessEngine, ResourceProvisioner,
    StubProvisioner,
};
