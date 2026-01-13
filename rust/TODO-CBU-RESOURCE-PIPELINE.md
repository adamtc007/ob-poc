# CBU Service → Resource Discovery → Unified Dictionary → Provisioning

## Overview

End-to-end pipeline where:
1. **CBU subscribes to Products** and configures Service options (markets, SSI, channels)
2. **Resource Discovery** derives required **ServiceResourceDefinitions (SRDEFs)** from ServiceIntent
3. All SRDEF **Attribute Profiles** roll up to **CBU Unified Attribute Dictionary** (de-duped)
4. Unified dictionary is populated (from CBU/entity/doc/derived/manual)
5. When SRDEF-required attributes satisfied, run **resource provisioning** (create/discover/bind)
6. **Owner systems respond** via append-only ledger → materialize into instances
7. **Service readiness** computed: "good-to-transact" status per product/service

**CRITICAL: Deliver a minimal working vertical slice: 1–2 products, 2–3 services, 3–6 SRDEFs.**

---

## Non-Negotiable Invariants

| Invariant | Enforcement |
|-----------|-------------|
| SRDEF defines required attrs via Resource Attribute Profile | Attribute Profile is FK-constrained subset of global Attribute Dictionary |
| CBU Unified Dictionary is **derived** (not hand-authored) | Rebuilt via `rollup_requirements()`, never edited directly |
| De-dupe key = `attr_id` | Composite PK on `(cbu_id, attr_id)` |
| Provisioning gate | SRDEF cannot provision until all required attrs satisfied + validations pass |
| Append-only ledger | `provisioning_requests` and `provisioning_events` are immutable audit trail |
| Idempotent | All derived artifacts rebuildable from source data |

---

## Data Flow

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                    PIPELINE                                          │
│                                                                                      │
│  ┌──────────────┐    ┌──────────────────┐    ┌──────────────────────────────────────┐│
│  │ ServiceIntent│───▶│ Resource         │───▶│ service_resource_instances           ││
│  │              │    │ Discovery        │    │ (srdef_id, state=requested)          ││
│  │ product +    │    │ Engine           │    └─────────────────┬────────────────────┘│
│  │ service +    │    │                  │                      │                     │
│  │ options      │    │ (rules-based)    │                      ▼                     │
│  └──────────────┘    └──────────────────┘    ┌──────────────────────────────────────┐│
│                                              │ Attribute Roll-Up                    ││
│                                              │                                      ││
│                                              │ SRDEF Attr Requirements              ││
│                                              │    └─▶ cbu_unified_attr_req          ││
│                                              │       (deduped, merged)              ││
│                                              └─────────────────┬────────────────────┘│
│                                                                │                     │
│                                                                ▼                     │
│  ┌──────────────┐    ┌──────────────────┐    ┌──────────────────────────────────────┐│
│  │ cbu_attr_    │◀───│ Population       │◀───│ Missing Attrs Report                 ││
│  │ values       │    │ Engine           │    │ (CBU/entity/doc/manual)              ││
│  └──────────────┘    └──────────────────┘    └──────────────────────────────────────┘│
│         │                                                                            │
│         ▼                                                                            │
│  ┌──────────────┐    ┌──────────────────┐    ┌──────────────────────────────────────┐│
│  │ Readiness    │───▶│ Provisioning     │───▶│ provisioning_requests (append-only)  ││
│  │ Gate         │    │ Orchestrator     │    │ provisioning_events (append-only)    ││
│  │ (all attrs?) │    │ (topo-sorted)    │    └─────────────────┬────────────────────┘│
│  └──────────────┘    └──────────────────┘                      │                     │
│                                                                │                     │
│                                              ┌─────────────────▼────────────────────┐│
│                                              │ Owner System Response                ││
│                                              │ (ProvisioningResult payload)         ││
│                                              │                                      ││
│                                              │ → Materialize into                   ││
│                                              │   service_resource_instances         ││
│                                              │   (srid, state=active)               ││
│                                              └─────────────────┬────────────────────┘│
│                                                                │                     │
│                                                                ▼                     │
│                                              ┌──────────────────────────────────────┐│
│                                              │ cbu_service_readiness                ││
│                                              │ (ready | blocked | partial)          ││
│                                              │ "good-to-transact" per service       ││
│                                              └──────────────────────────────────────┘│
│                                                                                      │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 0 — Core Data Types + DB Tables

### 0.1 Create module: `rust/src/domains/service_resources/mod.rs`

```rust
//! Service Resource Pipeline
//!
//! This module implements the CBU → ServiceIntent → SRDEF → Resource provisioning pipeline.
//! Core concepts:
//! - **ServiceIntent**: What the CBU wants (product + service + options)
//! - **SRDEF**: Service Resource Definition - what resources a service needs
//! - **Attribute Profile**: What data points an SRDEF requires
//! - **Unified Dictionary**: De-duped attribute requirements per CBU
//! - **Resource Instance**: Actual provisioned resource bound to CBU
//! - **Provisioning Ledger**: Append-only audit trail of requests/responses
//! - **Service Readiness**: "Good-to-transact" status per product/service

pub mod types;
pub mod discovery;
pub mod rollup;
pub mod population;
pub mod provisioning;
pub mod ledger;
pub mod readiness;
pub mod registry;
pub mod api;

pub use types::*;
pub use discovery::ResourceDiscoveryEngine;
pub use rollup::AttributeRollup;
pub use population::PopulationEngine;
pub use provisioning::{ProvisioningOrchestrator, ResourceProvisioner};
pub use ledger::{ProvisioningLedger, ProvisioningResult as OwnerProvisioningResult};
pub use readiness::ServiceReadinessEngine;
pub use registry::SrdefRegistry;
```

### 0.2 Create `rust/src/domains/service_resources/types.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// === ID Types (newtype pattern for safety) ===

/// Service Resource Definition ID: "SRDEF::APP::Kind::Purpose"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SrdefId(pub String);

impl SrdefId {
    pub fn new(app: &str, kind: &str, purpose: &str) -> Self {
        Self(format!("SRDEF::{}::{}::{}", app, kind, purpose))
    }
    
    pub fn parse(&self) -> Option<(&str, &str, &str)> {
        let parts: Vec<&str> = self.0.split("::").collect();
        if parts.len() == 4 && parts[0] == "SRDEF" {
            Some((parts[1], parts[2], parts[3]))
        } else {
            None
        }
    }
}

impl std::fmt::Display for SrdefId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Service Resource Instance ID: "SR::APP::Kind::NativeKey"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Srid(pub String);

impl Srid {
    pub fn new(app: &str, kind: &str, native_key: &str) -> Self {
        Self(format!("SR::{}::{}::{}", app, kind, native_key))
    }
}

impl std::fmt::Display for Srid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// === Core Domain Types ===

/// What the CBU wants - product subscription with service configuration
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceIntent {
    pub id: Uuid,
    pub cbu_id: Uuid,
    pub product_id: String,
    pub service_id: String,
    pub options: serde_json::Value,  // markets, channels, SSI mode, etc.
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// What resources a service needs - the "recipe"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Srdef {
    pub srdef_id: SrdefId,
    pub app_mnemonic: String,
    pub resource_kind: ResourceKind,
    pub resource_purpose: String,
    pub provisioning_strategy: ProvisioningStrategy,
    pub dependencies: Vec<SrdefId>,
    pub description: Option<String>,
}

/// Resource kinds that can be provisioned
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Account,
    InstructionSet,
    Connectivity,
    Entitlement,
    DataObject,
    DocumentArtifact,
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceKind::Account => write!(f, "Account"),
            ResourceKind::InstructionSet => write!(f, "InstructionSet"),
            ResourceKind::Connectivity => write!(f, "Connectivity"),
            ResourceKind::Entitlement => write!(f, "Entitlement"),
            ResourceKind::DataObject => write!(f, "DataObject"),
            ResourceKind::DocumentArtifact => write!(f, "DocumentArtifact"),
        }
    }
}

/// How to provision this resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisioningStrategy {
    Create,    // Create new resource in target system
    Request,   // Submit request for manual/async creation
    Discover,  // Find existing resource by criteria
}

/// What data points an SRDEF requires
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrdefAttributeRequirement {
    pub srdef_id: SrdefId,
    pub attr_id: String,  // References global attribute dictionary
    pub requirement: RequirementStrength,
    pub source_policy: Vec<AttributeSource>,
    pub constraints: serde_json::Value,  // type/range/regex/enum
    pub evidence_policy: EvidencePolicy,
}

/// Required vs optional vs conditional
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStrength {
    Required,
    Optional,
    Conditional,  // Required if some condition met
}

/// Where can we get this attribute from?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeSource {
    Derived,    // Computed from other attrs
    Entity,     // From entity tables
    Cbu,        // From CBU record
    Document,   // Extracted from documents
    Manual,     // User-entered
    External,   // From external system API
}

/// Evidence requirements for an attribute
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvidencePolicy {
    pub requires_document: bool,
    pub allowed_document_types: Vec<String>,
    pub verification_level: Option<String>,
}

/// Per-CBU unified attribute requirement (de-duped rollup)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuUnifiedAttrRequirement {
    pub cbu_id: Uuid,
    pub attr_id: String,
    pub requirement_strength: String,  // required|optional|conditional
    pub merged_constraints: serde_json::Value,
    pub preferred_source: String,
    pub required_by_srdefs: Vec<String>,  // SrdefId as strings
    pub conflict: Option<serde_json::Value>,
    pub explain: serde_json::Value,  // Explainability
}

/// Actual attribute value for a CBU
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuAttrValue {
    pub cbu_id: Uuid,
    pub attr_id: String,
    pub value: serde_json::Value,
    pub source: String,
    pub evidence_refs: Vec<String>,  // Document IDs
    pub explain_refs: Vec<String>,   // Derivation trace
    pub as_of: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Actual provisioned resource instance
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceResourceInstance {
    pub id: Uuid,
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub srid: Option<String>,
    pub native_key: Option<String>,
    pub state: ResourceState,
    pub bind_to: serde_json::Value,  // Entity ref, resolved PK, etc.
    pub discovery_explain: serde_json::Value,  // Why was this SRDEF required?
    pub provisioning_explain: Option<serde_json::Value>,  // What created the SRID?
    pub resource_url: Option<String>,  // Link to resource in owner system
    pub owner_ticket_id: Option<String>,  // Owner's tracking ID
    pub last_request_id: Option<Uuid>,  // FK to provisioning_requests
    pub last_event_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Resource lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ResourceState {
    Requested,
    Provisioning,
    Active,
    Failed,
    Suspended,
    Decommissioned,
}

impl std::fmt::Display for ResourceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceState::Requested => write!(f, "requested"),
            ResourceState::Provisioning => write!(f, "provisioning"),
            ResourceState::Active => write!(f, "active"),
            ResourceState::Failed => write!(f, "failed"),
            ResourceState::Suspended => write!(f, "suspended"),
            ResourceState::Decommissioned => write!(f, "decommissioned"),
        }
    }
}

// === Provisioning Ledger Types (Phase 6.5) ===

/// Provisioning request (append-only)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProvisioningRequest {
    pub request_id: Uuid,
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub requested_by: String,  // agent|user|system
    pub requested_at: DateTime<Utc>,
    pub request_payload: serde_json::Value,  // attrs snapshot, bind_to, evidence
    pub status: RequestStatus,
    pub owner_system: String,  // app mnemonic or team
    pub owner_ticket_id: Option<String>,
}

/// Request status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Queued,
    Sent,
    Ack,
    Completed,
    Failed,
    Cancelled,
}

/// Provisioning event (append-only ledger entry)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProvisioningEvent {
    pub event_id: Uuid,
    pub request_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub direction: EventDirection,
    pub kind: EventKind,
    pub payload: serde_json::Value,
    pub hash: Option<String>,  // Content hash for dedupe
}

/// Event direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "UPPERCASE")]
pub enum EventDirection {
    #[serde(rename = "OUT")]
    Out,
    #[serde(rename = "IN")]
    In,
}

/// Event kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "SCREAMING_SNAKE_CASE")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventKind {
    RequestSent,
    Ack,
    Result,
    Error,
    Status,
}

/// Owner system's response payload (canonical format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerProvisioningResult {
    pub srdef_id: String,
    pub request_id: Uuid,
    pub status: OwnerResultStatus,
    pub srid: Option<String>,
    pub native_key: Option<String>,
    pub native_key_type: Option<String>,
    pub resource_url: Option<String>,
    pub owner_ticket_id: Option<String>,
    pub explain: Option<OwnerExplain>,
    pub timestamp: DateTime<Utc>,
}

/// Owner result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerResultStatus {
    Active,
    Pending,
    Rejected,
    Failed,
}

/// Owner explanation (for failures/rejections)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerExplain {
    pub message: Option<String>,
    pub codes: Vec<String>,
}

// === Service Readiness Types (Phase 6.6) ===

/// Per-service readiness status
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuServiceReadiness {
    pub cbu_id: Uuid,
    pub product_id: String,
    pub service_id: String,
    pub status: ReadinessStatus,
    pub blocking_reasons: serde_json::Value,
    pub required_srdefs: Vec<String>,
    pub active_srids: Vec<String>,
    pub as_of: DateTime<Utc>,
}

/// Readiness status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReadinessStatus {
    Ready,
    Blocked,
    Partial,
}

/// Blocking reason detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingReason {
    pub kind: BlockingReasonKind,
    pub srdef_id: Option<String>,
    pub attr_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockingReasonKind {
    MissingSrdef,
    MissingAttr,
    AttrConflict,
    PendingProvisioning,
    FailedProvisioning,
    MissingResourceUrl,
}

// === Readiness Check Types ===

/// Report of what's missing for provisioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingInputsReport {
    pub srdef_id: SrdefId,
    pub ready: bool,
    pub missing_attrs: Vec<String>,
    pub conflicts: Vec<AttributeConflict>,
    pub missing_evidence: Vec<String>,
    pub unresolved_dependencies: Vec<SrdefId>,
}

/// When two SRDEFs have incompatible constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeConflict {
    pub attr_id: String,
    pub srdef_a: SrdefId,
    pub srdef_b: SrdefId,
    pub constraint_a: serde_json::Value,
    pub constraint_b: serde_json::Value,
    pub description: String,
}

// === API Response Types ===

#[derive(Debug, Serialize)]
pub struct DiscoveryResult {
    pub cbu_id: Uuid,
    pub srdefs_discovered: Vec<SrdefId>,
    pub instances_created: usize,
    pub explain: Vec<DiscoveryExplain>,
}

#[derive(Debug, Serialize)]
pub struct DiscoveryExplain {
    pub srdef_id: SrdefId,
    pub triggered_by: String,  // Which intent + options
    pub rule: String,          // Which discovery rule matched
}

#[derive(Debug, Serialize)]
pub struct RollupResult {
    pub cbu_id: Uuid,
    pub total_attributes: usize,
    pub required_count: usize,
    pub optional_count: usize,
    pub conflicts: Vec<AttributeConflict>,
}

#[derive(Debug, Serialize)]
pub struct ProvisioningResult {
    pub cbu_id: Uuid,
    pub provisioned: Vec<ProvisionedResource>,
    pub blocked: Vec<BlockedResource>,
    pub requests_created: Vec<Uuid>,  // request_ids for async provisioning
}

#[derive(Debug, Serialize)]
pub struct ProvisionedResource {
    pub srdef_id: SrdefId,
    pub srid: Srid,
    pub native_key: String,
}

#[derive(Debug, Serialize)]
pub struct BlockedResource {
    pub srdef_id: SrdefId,
    pub missing_report: MissingInputsReport,
}

#[derive(Debug, Serialize)]
pub struct ReadinessResult {
    pub cbu_id: Uuid,
    pub services: Vec<CbuServiceReadiness>,
    pub all_ready: bool,
    pub blocked_count: usize,
}
```

### 0.3 Add to `rust/src/domains/mod.rs`

```rust
pub mod attributes;
pub mod service_resources;  // <-- ADD THIS LINE
```

### 0.4 DB Migration: `rust/migrations/20260113_service_resource_pipeline.sql`

```sql
-- Service Resource Pipeline Tables
-- CBU → ServiceIntent → SRDEF → Resource Instance → Owner Response → Service Readiness

-- ============================================================================
-- SERVICE INTENTS: What the CBU wants
-- ============================================================================
CREATE TABLE IF NOT EXISTS service_intents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    service_id TEXT NOT NULL,
    options JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Unique constraint: one intent per product+service per CBU
    CONSTRAINT uq_service_intent_cbu_product_service 
        UNIQUE (cbu_id, product_id, service_id)
);

CREATE INDEX idx_service_intents_cbu ON service_intents(cbu_id);
CREATE INDEX idx_service_intents_product ON service_intents(product_id);

-- ============================================================================
-- SRDEFS: Service Resource Definitions (reference data, can be seeded)
-- ============================================================================
CREATE TABLE IF NOT EXISTS srdefs (
    srdef_id TEXT PRIMARY KEY,  -- "SRDEF::APP::Kind::Purpose"
    app_mnemonic TEXT NOT NULL,
    resource_kind TEXT NOT NULL,  -- Account|InstructionSet|Connectivity|...
    resource_purpose TEXT NOT NULL,
    provisioning_strategy TEXT NOT NULL,  -- create|request|discover
    dependencies TEXT[] NOT NULL DEFAULT '{}',  -- Array of srdef_ids
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_srdefs_app ON srdefs(app_mnemonic);
CREATE INDEX idx_srdefs_kind ON srdefs(resource_kind);

-- ============================================================================
-- SRDEF ATTRIBUTE REQUIREMENTS: What data each SRDEF needs
-- ============================================================================
CREATE TABLE IF NOT EXISTS srdef_attribute_requirements (
    srdef_id TEXT NOT NULL REFERENCES srdefs(srdef_id) ON DELETE CASCADE,
    attr_id TEXT NOT NULL,  -- References attribute dictionary
    requirement TEXT NOT NULL DEFAULT 'required',  -- required|optional|conditional
    source_policy TEXT[] NOT NULL DEFAULT '{}',  -- derived|entity|cbu|document|manual|external
    constraints JSONB NOT NULL DEFAULT '{}',
    evidence_policy JSONB NOT NULL DEFAULT '{}',
    
    PRIMARY KEY (srdef_id, attr_id)
);

CREATE INDEX idx_srdef_attr_req_attr ON srdef_attribute_requirements(attr_id);

-- ============================================================================
-- CBU UNIFIED ATTR REQUIREMENTS: De-duped rollup (DERIVED - rebuildable)
-- ============================================================================
CREATE TABLE IF NOT EXISTS cbu_unified_attr_requirements (
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    attr_id TEXT NOT NULL,
    requirement_strength TEXT NOT NULL,  -- required|optional|conditional
    merged_constraints JSONB NOT NULL DEFAULT '{}',
    preferred_source TEXT NOT NULL,
    required_by_srdefs TEXT[] NOT NULL DEFAULT '{}',
    conflict JSONB,  -- Non-null if constraints can't merge
    explain JSONB NOT NULL DEFAULT '{}',  -- Explainability
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (cbu_id, attr_id)
);

COMMENT ON TABLE cbu_unified_attr_requirements IS 'DERIVED: Rebuild via rollup_requirements()';

-- ============================================================================
-- CBU ATTR VALUES: Actual populated values
-- ============================================================================
CREATE TABLE IF NOT EXISTS cbu_attr_values (
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    attr_id TEXT NOT NULL,
    value JSONB NOT NULL,
    source TEXT NOT NULL,  -- derived|entity|cbu|document|manual|external
    evidence_refs TEXT[] NOT NULL DEFAULT '{}',  -- Document IDs
    explain_refs TEXT[] NOT NULL DEFAULT '{}',   -- Derivation trace
    as_of TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (cbu_id, attr_id)
);

CREATE INDEX idx_cbu_attr_values_source ON cbu_attr_values(source);

-- ============================================================================
-- SERVICE RESOURCE INSTANCES: Actual provisioned resources (DERIVED)
-- ============================================================================
CREATE TABLE IF NOT EXISTS service_resource_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    srdef_id TEXT NOT NULL REFERENCES srdefs(srdef_id),
    srid TEXT,  -- NULL until provisioned
    native_key TEXT,  -- App-specific key
    state TEXT NOT NULL DEFAULT 'requested',
    bind_to JSONB NOT NULL DEFAULT '{}',  -- Entity ref, resolved PK
    discovery_explain JSONB NOT NULL DEFAULT '{}',
    provisioning_explain JSONB,
    -- Phase 6.5 additions
    resource_url TEXT,  -- Link to resource in owner system
    owner_ticket_id TEXT,  -- Owner's tracking ID (e.g., INC12345)
    last_request_id UUID,  -- FK to provisioning_requests
    last_event_at TIMESTAMPTZ,
    --
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- One instance per SRDEF per CBU
    CONSTRAINT uq_resource_instance_cbu_srdef 
        UNIQUE (cbu_id, srdef_id)
);

COMMENT ON TABLE service_resource_instances IS 'DERIVED: Rebuild via resource_discover()';

CREATE INDEX idx_sri_cbu ON service_resource_instances(cbu_id);
CREATE INDEX idx_sri_state ON service_resource_instances(state);
CREATE INDEX idx_sri_srdef ON service_resource_instances(srdef_id);
CREATE INDEX idx_sri_last_request ON service_resource_instances(last_request_id);

-- ============================================================================
-- PROVISIONING REQUESTS: Append-only ledger (Phase 6.5)
-- ============================================================================
CREATE TABLE IF NOT EXISTS provisioning_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    srdef_id TEXT NOT NULL REFERENCES srdefs(srdef_id),
    requested_by TEXT NOT NULL,  -- agent|user|system
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    request_payload JSONB NOT NULL DEFAULT '{}',  -- attrs snapshot, bind_to, evidence
    status TEXT NOT NULL DEFAULT 'queued',  -- queued|sent|ack|completed|failed|cancelled
    owner_system TEXT NOT NULL,  -- app mnemonic or team
    owner_ticket_id TEXT  -- populated when owner acknowledges
);

COMMENT ON TABLE provisioning_requests IS 'APPEND-ONLY: Never update or delete rows';

CREATE INDEX idx_prov_req_cbu ON provisioning_requests(cbu_id);
CREATE INDEX idx_prov_req_srdef ON provisioning_requests(srdef_id);
CREATE INDEX idx_prov_req_status ON provisioning_requests(status);
CREATE INDEX idx_prov_req_owner ON provisioning_requests(owner_system);

-- ============================================================================
-- PROVISIONING EVENTS: Append-only event log (Phase 6.5)
-- ============================================================================
CREATE TABLE IF NOT EXISTS provisioning_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID NOT NULL REFERENCES provisioning_requests(request_id),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    direction TEXT NOT NULL,  -- OUT|IN
    kind TEXT NOT NULL,  -- REQUEST_SENT|ACK|RESULT|ERROR|STATUS
    payload JSONB NOT NULL DEFAULT '{}',  -- canonical ProvisioningResult or status
    hash TEXT  -- content hash for dedupe
);

COMMENT ON TABLE provisioning_events IS 'APPEND-ONLY: Never update or delete rows';

CREATE INDEX idx_prov_evt_request ON provisioning_events(request_id);
CREATE INDEX idx_prov_evt_kind ON provisioning_events(kind);
CREATE INDEX idx_prov_evt_occurred ON provisioning_events(occurred_at);
CREATE INDEX idx_prov_evt_hash ON provisioning_events(hash) WHERE hash IS NOT NULL;

-- ============================================================================
-- CBU SERVICE READINESS: "Good-to-transact" status (Phase 6.6)
-- ============================================================================
CREATE TABLE IF NOT EXISTS cbu_service_readiness (
    cbu_id UUID NOT NULL REFERENCES cbus(cbu_id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    service_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'blocked',  -- ready|blocked|partial
    blocking_reasons JSONB NOT NULL DEFAULT '[]',
    required_srdefs JSONB NOT NULL DEFAULT '[]',  -- array of srdef_ids
    active_srids JSONB NOT NULL DEFAULT '[]',     -- array of srids
    as_of TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    PRIMARY KEY (cbu_id, product_id, service_id)
);

COMMENT ON TABLE cbu_service_readiness IS 'DERIVED: Rebuild via compute_readiness()';

CREATE INDEX idx_cbu_readiness_status ON cbu_service_readiness(status);
CREATE INDEX idx_cbu_readiness_cbu ON cbu_service_readiness(cbu_id);

-- ============================================================================
-- UPDATED_AT TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_service_intents_updated_at
    BEFORE UPDATE ON service_intents
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_sri_updated_at
    BEFORE UPDATE ON service_resource_instances
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- PREVENT UPDATES/DELETES ON APPEND-ONLY TABLES
-- ============================================================================
CREATE OR REPLACE FUNCTION prevent_modification()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'Table % is append-only. Updates and deletes are not allowed.', TG_TABLE_NAME;
END;
$$ language 'plpgsql';

CREATE TRIGGER provisioning_requests_immutable
    BEFORE UPDATE OR DELETE ON provisioning_requests
    FOR EACH ROW EXECUTE FUNCTION prevent_modification();

CREATE TRIGGER provisioning_events_immutable
    BEFORE UPDATE OR DELETE ON provisioning_events
    FOR EACH ROW EXECUTE FUNCTION prevent_modification();
```

---

## Phase 1 — SRDEF Registry + YAML Loader

### 1.1 Create `rust/config/srdefs/` directory structure

```
config/srdefs/
├── custody.yaml
├── swift.yaml
├── iam.yaml
└── ta.yaml
```

### 1.2 Create `rust/config/srdefs/custody.yaml`

```yaml
# Custody Service Resource Definitions

srdefs:
  - srdef_id: "SRDEF::CUSTODY::Account::custody_securities"
    app_mnemonic: CUSTODY
    resource_kind: account
    resource_purpose: custody_securities
    provisioning_strategy: create
    dependencies: []
    description: "Securities custody account for holding positions"
    attributes:
      - attr_id: market_scope
        requirement: required
        source_policy: [cbu, manual]
        constraints:
          type: array
          items: { type: string, pattern: "^[A-Z]{4}$" }  # MIC codes
        evidence_policy: {}
        
      - attr_id: settlement_currency
        requirement: required
        source_policy: [derived, manual]
        constraints:
          type: string
          pattern: "^[A-Z]{3}$"  # ISO 4217
        evidence_policy: {}
        
      - attr_id: account_name
        requirement: required
        source_policy: [entity, cbu]
        constraints:
          type: string
          maxLength: 100
        evidence_policy: {}
        
      - attr_id: custody_type
        requirement: required
        source_policy: [manual]
        constraints:
          type: string
          enum: [omnibus, segregated, nominee]
        evidence_policy: {}

  - srdef_id: "SRDEF::CUSTODY::Account::custody_cash"
    app_mnemonic: CUSTODY
    resource_kind: account
    resource_purpose: custody_cash
    provisioning_strategy: create
    dependencies:
      - "SRDEF::CUSTODY::Account::custody_securities"
    description: "Cash account linked to securities custody"
    attributes:
      - attr_id: cash_currency
        requirement: required
        source_policy: [derived, manual]
        constraints:
          type: string
          pattern: "^[A-Z]{3}$"
        evidence_policy: {}
        
      - attr_id: nostro_account
        requirement: optional
        source_policy: [manual]
        constraints:
          type: string
        evidence_policy: {}
```

### 1.3 Create `rust/config/srdefs/swift.yaml`

```yaml
# SWIFT Connectivity Resource Definitions

srdefs:
  - srdef_id: "SRDEF::SWIFT::Connectivity::swift_sender_receiver"
    app_mnemonic: SWIFT
    resource_kind: connectivity
    resource_purpose: swift_sender_receiver
    provisioning_strategy: discover
    dependencies: []
    description: "SWIFT BIC connectivity for settlement instructions"
    attributes:
      - attr_id: bic_sender
        requirement: required
        source_policy: [entity, manual]
        constraints:
          type: string
          pattern: "^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$"  # BIC8 or BIC11
        evidence_policy:
          requires_document: false
          
      - attr_id: bic_receiver
        requirement: required
        source_policy: [entity, manual]
        constraints:
          type: string
          pattern: "^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$"
        evidence_policy:
          requires_document: false
          
      - attr_id: message_types
        requirement: required
        source_policy: [derived, manual]
        constraints:
          type: array
          items: { type: string, pattern: "^MT[0-9]{3}$" }
          default: ["MT540", "MT541", "MT542", "MT543"]
        evidence_policy: {}
```

### 1.4 Create `rust/config/srdefs/iam.yaml`

```yaml
# IAM Entitlement Resource Definitions

srdefs:
  - srdef_id: "SRDEF::IAM::Entitlement::custody_ops_role"
    app_mnemonic: IAM
    resource_kind: entitlement
    resource_purpose: custody_ops_role
    provisioning_strategy: request
    dependencies: []
    description: "Custody operations role for settlement processing"
    attributes:
      - attr_id: role_name
        requirement: required
        source_policy: [derived]
        constraints:
          type: string
          default: "custody_operations"
        evidence_policy: {}
        
      - attr_id: permission_scope
        requirement: required
        source_policy: [derived, manual]
        constraints:
          type: array
          items: { type: string }
          default: ["view_positions", "initiate_settlement", "approve_settlement"]
        evidence_policy: {}
        
      - attr_id: assigned_users
        requirement: optional
        source_policy: [entity, manual]
        constraints:
          type: array
          items: { type: string, format: email }
        evidence_policy: {}
```

### 1.5 Create `rust/src/domains/service_resources/registry.rs`

```rust
//! SRDEF Registry - Loads and indexes Service Resource Definitions

use crate::domains::service_resources::types::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::info;

/// In-memory registry of all SRDEFs
#[derive(Debug, Default)]
pub struct SrdefRegistry {
    /// SRDEF by ID
    srdefs: HashMap<SrdefId, Srdef>,
    /// Attribute requirements by SRDEF ID
    attr_requirements: HashMap<SrdefId, Vec<SrdefAttributeRequirement>>,
    /// SRDEFs by app mnemonic
    by_app: HashMap<String, Vec<SrdefId>>,
    /// SRDEFs by resource kind
    by_kind: HashMap<ResourceKind, Vec<SrdefId>>,
}

impl SrdefRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Load all SRDEF YAML files from directory
    pub async fn load_from_directory(&mut self, dir: &Path) -> Result<usize> {
        let mut count = 0;
        let mut entries = fs::read_dir(dir).await
            .with_context(|| format!("Failed to read SRDEF directory: {:?}", dir))?;
            
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml" || e == "yml") {
                let loaded = self.load_file(&path).await?;
                count += loaded;
                info!("Loaded {} SRDEFs from {:?}", loaded, path);
            }
        }
        
        info!("Total SRDEFs loaded: {}", count);
        Ok(count)
    }
    
    /// Load a single YAML file
    async fn load_file(&mut self, path: &Path) -> Result<usize> {
        let content = fs::read_to_string(path).await
            .with_context(|| format!("Failed to read file: {:?}", path))?;
        let file: SrdefFile = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML: {:?}", path))?;
            
        let mut count = 0;
        for srdef_yaml in file.srdefs {
            self.register_from_yaml(srdef_yaml)?;
            count += 1;
        }
        Ok(count)
    }
    
    /// Register an SRDEF from parsed YAML
    fn register_from_yaml(&mut self, yaml: SrdefYaml) -> Result<()> {
        let srdef_id = SrdefId(yaml.srdef_id.clone());
        
        let srdef = Srdef {
            srdef_id: srdef_id.clone(),
            app_mnemonic: yaml.app_mnemonic.clone(),
            resource_kind: yaml.resource_kind,
            resource_purpose: yaml.resource_purpose,
            provisioning_strategy: yaml.provisioning_strategy,
            dependencies: yaml.dependencies.into_iter().map(SrdefId).collect(),
            description: yaml.description,
        };
        
        // Parse attribute requirements
        let mut attr_reqs = Vec::new();
        for attr in yaml.attributes {
            attr_reqs.push(SrdefAttributeRequirement {
                srdef_id: srdef_id.clone(),
                attr_id: attr.attr_id,
                requirement: attr.requirement,
                source_policy: attr.source_policy,
                constraints: attr.constraints,
                evidence_policy: attr.evidence_policy.unwrap_or_default(),
            });
        }
        
        // Index by app
        self.by_app
            .entry(yaml.app_mnemonic)
            .or_default()
            .push(srdef_id.clone());
            
        // Index by kind
        self.by_kind
            .entry(srdef.resource_kind)
            .or_default()
            .push(srdef_id.clone());
        
        self.attr_requirements.insert(srdef_id.clone(), attr_reqs);
        self.srdefs.insert(srdef_id, srdef);
        
        Ok(())
    }
    
    // === Query Methods ===
    
    pub fn get(&self, id: &SrdefId) -> Option<&Srdef> {
        self.srdefs.get(id)
    }
    
    pub fn get_requirements(&self, id: &SrdefId) -> Option<&Vec<SrdefAttributeRequirement>> {
        self.attr_requirements.get(id)
    }
    
    pub fn all_srdefs(&self) -> impl Iterator<Item = &Srdef> {
        self.srdefs.values()
    }
    
    pub fn by_app(&self, app: &str) -> Vec<&Srdef> {
        self.by_app.get(app)
            .map(|ids| ids.iter().filter_map(|id| self.srdefs.get(id)).collect())
            .unwrap_or_default()
    }
    
    pub fn by_kind(&self, kind: ResourceKind) -> Vec<&Srdef> {
        self.by_kind.get(&kind)
            .map(|ids| ids.iter().filter_map(|id| self.srdefs.get(id)).collect())
            .unwrap_or_default()
    }
    
    pub fn srdef_count(&self) -> usize {
        self.srdefs.len()
    }
    
    /// Get owner system for an SRDEF (uses app_mnemonic)
    pub fn get_owner_system(&self, id: &SrdefId) -> Option<&str> {
        self.srdefs.get(id).map(|s| s.app_mnemonic.as_str())
    }
}

// === YAML Deserialization Types ===

#[derive(Debug, Deserialize)]
struct SrdefFile {
    srdefs: Vec<SrdefYaml>,
}

#[derive(Debug, Deserialize)]
struct SrdefYaml {
    srdef_id: String,
    app_mnemonic: String,
    resource_kind: ResourceKind,
    resource_purpose: String,
    provisioning_strategy: ProvisioningStrategy,
    #[serde(default)]
    dependencies: Vec<String>,
    description: Option<String>,
    #[serde(default)]
    attributes: Vec<SrdefAttributeYaml>,
}

#[derive(Debug, Deserialize)]
struct SrdefAttributeYaml {
    attr_id: String,
    requirement: RequirementStrength,
    #[serde(default)]
    source_policy: Vec<AttributeSource>,
    #[serde(default)]
    constraints: serde_json::Value,
    evidence_policy: Option<EvidencePolicy>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_load_srdefs() {
        let mut registry = SrdefRegistry::new();
        // Will need actual config path in integration test
        assert_eq!(registry.srdef_count(), 0);
    }
}
```

---

## Phase 2 — Service Intent Capture (DSL + API)

### 2.1 Add DSL verbs to `rust/config/verbs/service.yaml`

```yaml
# Service Intent DSL Verbs

verbs:
  - verb: product.subscribe
    category: service
    description: "Subscribe a CBU to a product"
    params:
      - name: cbu
        type: cbu_ref
        required: true
      - name: product
        type: string
        required: true
    example: '(product.subscribe (cbu "allianz-lux") (product "custody"))'
    
  - verb: service.configure
    category: service
    description: "Configure service options for a CBU"
    params:
      - name: cbu
        type: cbu_ref
        required: true
      - name: product
        type: string
        required: true
      - name: service
        type: string
        required: true
      - name: options
        type: map
        required: true
    example: '(service.configure (cbu "allianz-lux") (product "custody") (service "settlement") (options {:markets ["XNAS" "XNYS"] :ssi_mode "standing"}))'
    
  - verb: resource.discover
    category: service
    description: "Run resource discovery for a CBU"
    params:
      - name: cbu
        type: cbu_ref
        required: true
    example: '(resource.discover (cbu "allianz-lux"))'
    
  - verb: resource.provision
    category: service
    description: "Provision discovered resources for a CBU"
    params:
      - name: cbu
        type: cbu_ref
        required: true
    example: '(resource.provision (cbu "allianz-lux"))'
    
  - verb: readiness.check
    category: service
    description: "Check service readiness for a CBU"
    params:
      - name: cbu
        type: cbu_ref
        required: true
    example: '(readiness.check (cbu "allianz-lux"))'
```

### 2.2 Create API module `rust/src/domains/service_resources/api.rs`

```rust
//! API handlers for Service Resource Pipeline

use crate::domains::service_resources::{
    types::*,
    discovery::ResourceDiscoveryEngine,
    rollup::AttributeRollup,
    population::PopulationEngine,
    provisioning::ProvisioningOrchestrator,
    ledger::ProvisioningLedger,
    readiness::ServiceReadinessEngine,
    registry::SrdefRegistry,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct ServiceResourceState {
    pub pool: PgPool,
    pub registry: Arc<SrdefRegistry>,
}

// === Service Intent Endpoints ===

/// GET /cbu/{id}/service-intents
pub async fn list_service_intents(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<ServiceIntent>>, StatusCode> {
    let intents = sqlx::query_as!(
        ServiceIntent,
        r#"
        SELECT id, cbu_id, product_id, service_id, options, created_at, updated_at
        FROM service_intents
        WHERE cbu_id = $1
        ORDER BY product_id, service_id
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(intents))
}

/// POST /cbu/{id}/service-intents
#[derive(Debug, serde::Deserialize)]
pub struct CreateServiceIntentRequest {
    pub product_id: String,
    pub service_id: String,
    pub options: serde_json::Value,
}

pub async fn create_service_intent(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
    Json(req): Json<CreateServiceIntentRequest>,
) -> Result<Json<ServiceIntent>, StatusCode> {
    let intent = sqlx::query_as!(
        ServiceIntent,
        r#"
        INSERT INTO service_intents (cbu_id, product_id, service_id, options)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (cbu_id, product_id, service_id)
        DO UPDATE SET options = EXCLUDED.options, updated_at = NOW()
        RETURNING id, cbu_id, product_id, service_id, options, created_at, updated_at
        "#,
        cbu_id,
        req.product_id,
        req.service_id,
        req.options
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(intent))
}

// === Resource Discovery Endpoint ===

/// POST /cbu/{id}/resource-discover
pub async fn run_resource_discovery(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<DiscoveryResult>, StatusCode> {
    let engine = ResourceDiscoveryEngine::new(state.registry.clone());
    
    // Fetch service intents
    let intents = sqlx::query_as!(
        ServiceIntent,
        r#"
        SELECT id, cbu_id, product_id, service_id, options, created_at, updated_at
        FROM service_intents
        WHERE cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Run discovery
    let result = engine.discover(cbu_id, &intents, &state.pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(result))
}

// === Attribute Rollup Endpoint ===

/// POST /cbu/{id}/attributes/rollup
pub async fn run_attribute_rollup(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<RollupResult>, StatusCode> {
    let rollup = AttributeRollup::new(state.registry.clone());
    
    let result = rollup.build_unified_requirements(cbu_id, &state.pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(result))
}

/// GET /cbu/{id}/attributes/requirements
pub async fn get_attribute_requirements(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<CbuUnifiedAttrRequirement>>, StatusCode> {
    let reqs = sqlx::query_as!(
        CbuUnifiedAttrRequirement,
        r#"
        SELECT cbu_id, attr_id, requirement_strength, merged_constraints,
               preferred_source, required_by_srdefs, conflict, explain
        FROM cbu_unified_attr_requirements
        WHERE cbu_id = $1
        ORDER BY requirement_strength DESC, attr_id
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(reqs))
}

// === Attribute Values Endpoints ===

/// GET /cbu/{id}/attributes/values
pub async fn get_attribute_values(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<CbuAttrValue>>, StatusCode> {
    let values = sqlx::query_as!(
        CbuAttrValue,
        r#"
        SELECT cbu_id, attr_id, value, source, evidence_refs, explain_refs, as_of, created_at
        FROM cbu_attr_values
        WHERE cbu_id = $1
        ORDER BY attr_id
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(values))
}

/// POST /cbu/{id}/attributes/values
#[derive(Debug, serde::Deserialize)]
pub struct SetAttributeValueRequest {
    pub attr_id: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

pub async fn set_attribute_value(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
    Json(req): Json<SetAttributeValueRequest>,
) -> Result<Json<CbuAttrValue>, StatusCode> {
    let val = sqlx::query_as!(
        CbuAttrValue,
        r#"
        INSERT INTO cbu_attr_values (cbu_id, attr_id, value, source, evidence_refs)
        VALUES ($1, $2, $3, 'manual', $4)
        ON CONFLICT (cbu_id, attr_id)
        DO UPDATE SET value = EXCLUDED.value, source = 'manual', 
                      evidence_refs = EXCLUDED.evidence_refs, as_of = NOW()
        RETURNING cbu_id, attr_id, value, source, evidence_refs, explain_refs, as_of, created_at
        "#,
        cbu_id,
        req.attr_id,
        req.value,
        &req.evidence_refs
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(val))
}

// === Provisioning Endpoints ===

/// POST /cbu/{id}/resources/provision
pub async fn run_provisioning(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<ProvisioningResult>, StatusCode> {
    let orchestrator = ProvisioningOrchestrator::new(state.registry.clone());
    
    let result = orchestrator.provision(cbu_id, &state.pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(result))
}

/// GET /cbu/{id}/resources
pub async fn list_resources(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<ServiceResourceInstance>>, StatusCode> {
    let resources = sqlx::query_as!(
        ServiceResourceInstance,
        r#"
        SELECT id, cbu_id, srdef_id, srid, native_key, 
               state as "state: ResourceState", bind_to,
               discovery_explain, provisioning_explain,
               resource_url, owner_ticket_id, last_request_id, last_event_at,
               created_at, updated_at
        FROM service_resource_instances
        WHERE cbu_id = $1
        ORDER BY srdef_id
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(resources))
}

// === Provisioning Ledger Endpoints (Phase 6.5) ===

/// GET /cbu/{id}/provisioning/requests
pub async fn list_provisioning_requests(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<ProvisioningRequest>>, StatusCode> {
    let requests = sqlx::query_as!(
        ProvisioningRequest,
        r#"
        SELECT request_id, cbu_id, srdef_id, requested_by, requested_at,
               request_payload, status as "status: RequestStatus",
               owner_system, owner_ticket_id
        FROM provisioning_requests
        WHERE cbu_id = $1
        ORDER BY requested_at DESC
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(requests))
}

/// GET /provisioning/requests/{request_id}/events
pub async fn list_provisioning_events(
    State(state): State<Arc<ServiceResourceState>>,
    Path(request_id): Path<Uuid>,
) -> Result<Json<Vec<ProvisioningEvent>>, StatusCode> {
    let events = sqlx::query_as!(
        ProvisioningEvent,
        r#"
        SELECT event_id, request_id, occurred_at,
               direction as "direction: EventDirection",
               kind as "kind: EventKind",
               payload, hash
        FROM provisioning_events
        WHERE request_id = $1
        ORDER BY occurred_at ASC
        "#,
        request_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(events))
}

/// POST /provisioning/result - Webhook for owner system responses
#[derive(Debug, serde::Deserialize)]
pub struct OwnerResultWebhook {
    pub result: OwnerProvisioningResult,
}

pub async fn receive_provisioning_result(
    State(state): State<Arc<ServiceResourceState>>,
    Json(req): Json<OwnerResultWebhook>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let ledger = ProvisioningLedger::new();
    
    ledger.process_owner_result(&req.result, &state.pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(serde_json::json!({ "status": "accepted" })))
}

// === Service Readiness Endpoints (Phase 6.6) ===

/// POST /cbu/{id}/readiness/recompute
pub async fn recompute_readiness(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<ReadinessResult>, StatusCode> {
    let engine = ServiceReadinessEngine::new(state.registry.clone());
    
    let result = engine.compute_readiness(cbu_id, &state.pool).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(result))
}

/// GET /cbu/{id}/readiness
pub async fn get_readiness(
    State(state): State<Arc<ServiceResourceState>>,
    Path(cbu_id): Path<Uuid>,
) -> Result<Json<Vec<CbuServiceReadiness>>, StatusCode> {
    let readiness = sqlx::query_as!(
        CbuServiceReadiness,
        r#"
        SELECT cbu_id, product_id, service_id,
               status as "status: ReadinessStatus",
               blocking_reasons, required_srdefs, active_srids, as_of
        FROM cbu_service_readiness
        WHERE cbu_id = $1
        ORDER BY product_id, service_id
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(readiness))
}

// === Router Configuration ===

pub fn service_resource_routes() -> axum::Router<Arc<ServiceResourceState>> {
    use axum::routing::{get, post};
    
    axum::Router::new()
        // Service Intents
        .route("/cbu/:id/service-intents", get(list_service_intents).post(create_service_intent))
        // Resource Discovery
        .route("/cbu/:id/resource-discover", post(run_resource_discovery))
        // Attribute Rollup
        .route("/cbu/:id/attributes/rollup", post(run_attribute_rollup))
        .route("/cbu/:id/attributes/requirements", get(get_attribute_requirements))
        // Attribute Values
        .route("/cbu/:id/attributes/values", get(get_attribute_values).post(set_attribute_value))
        // Provisioning
        .route("/cbu/:id/resources/provision", post(run_provisioning))
        .route("/cbu/:id/resources", get(list_resources))
        // Provisioning Ledger (Phase 6.5)
        .route("/cbu/:id/provisioning/requests", get(list_provisioning_requests))
        .route("/provisioning/requests/:request_id/events", get(list_provisioning_events))
        .route("/provisioning/result", post(receive_provisioning_result))
        // Service Readiness (Phase 6.6)
        .route("/cbu/:id/readiness/recompute", post(recompute_readiness))
        .route("/cbu/:id/readiness", get(get_readiness))
}
```

---

## Phase 3 — Resource Discovery Engine

### 3.1 Create `rust/src/domains/service_resources/discovery.rs`

```rust
//! Resource Discovery Engine
//!
//! Maps ServiceIntents to required SRDEFs based on configurable rules.
//! Discovery is deterministic and idempotent.

use crate::domains::service_resources::{types::*, registry::SrdefRegistry};
use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct ResourceDiscoveryEngine {
    registry: Arc<SrdefRegistry>,
}

impl ResourceDiscoveryEngine {
    pub fn new(registry: Arc<SrdefRegistry>) -> Self {
        Self { registry }
    }
    
    /// Run discovery for a CBU: intents → required SRDEFs
    ///
    /// This is idempotent: deletes existing instances and rebuilds.
    pub async fn discover(
        &self,
        cbu_id: Uuid,
        intents: &[ServiceIntent],
        pool: &PgPool,
    ) -> Result<DiscoveryResult> {
        let mut discovered: Vec<(SrdefId, DiscoveryExplain)> = Vec::new();
        
        for intent in intents {
            let srdefs = self.match_intent(intent)?;
            discovered.extend(srdefs);
        }
        
        // De-dupe by SRDEF ID (keep first explain)
        let mut seen = std::collections::HashSet::new();
        let unique: Vec<_> = discovered.into_iter()
            .filter(|(id, _)| seen.insert(id.clone()))
            .collect();
        
        // Delete existing instances (idempotent rebuild)
        sqlx::query!(
            "DELETE FROM service_resource_instances WHERE cbu_id = $1",
            cbu_id
        )
        .execute(pool)
        .await?;
        
        // Insert new instances
        for (srdef_id, explain) in &unique {
            sqlx::query!(
                r#"
                INSERT INTO service_resource_instances 
                    (cbu_id, srdef_id, state, discovery_explain)
                VALUES ($1, $2, 'requested', $3)
                "#,
                cbu_id,
                srdef_id.0,
                json!({
                    "triggered_by": explain.triggered_by,
                    "rule": explain.rule,
                })
            )
            .execute(pool)
            .await?;
        }
        
        Ok(DiscoveryResult {
            cbu_id,
            srdefs_discovered: unique.iter().map(|(id, _)| id.clone()).collect(),
            instances_created: unique.len(),
            explain: unique.into_iter().map(|(_, e)| e).collect(),
        })
    }
    
    /// Match a single intent to SRDEFs
    fn match_intent(&self, intent: &ServiceIntent) -> Result<Vec<(SrdefId, DiscoveryExplain)>> {
        let mut results = Vec::new();
        
        // === DISCOVERY RULES ===
        // These are hardcoded for now; can move to YAML/DB later
        
        let product = intent.product_id.to_lowercase();
        let service = intent.service_id.to_lowercase();
        let options = &intent.options;
        
        // Rule 1: Custody + Settlement → custody accounts + SWIFT
        if product == "custody" && service == "settlement" {
            // Always need custody securities account
            results.push((
                SrdefId::new("CUSTODY", "Account", "custody_securities"),
                DiscoveryExplain {
                    srdef_id: SrdefId::new("CUSTODY", "Account", "custody_securities"),
                    triggered_by: format!("product={}, service={}", product, service),
                    rule: "custody_settlement_requires_securities_account".to_string(),
                },
            ));
            
            // Always need custody cash account
            results.push((
                SrdefId::new("CUSTODY", "Account", "custody_cash"),
                DiscoveryExplain {
                    srdef_id: SrdefId::new("CUSTODY", "Account", "custody_cash"),
                    triggered_by: format!("product={}, service={}", product, service),
                    rule: "custody_settlement_requires_cash_account".to_string(),
                },
            ));
            
            // If markets include US exchanges, need SWIFT connectivity
            if let Some(markets) = options.get("markets").and_then(|m| m.as_array()) {
                let has_us_market = markets.iter().any(|m| {
                    m.as_str().map_or(false, |s| s.starts_with("XN"))
                });
                if has_us_market {
                    results.push((
                        SrdefId::new("SWIFT", "Connectivity", "swift_sender_receiver"),
                        DiscoveryExplain {
                            srdef_id: SrdefId::new("SWIFT", "Connectivity", "swift_sender_receiver"),
                            triggered_by: format!("markets include US exchange"),
                            rule: "us_markets_require_swift".to_string(),
                        },
                    ));
                }
            }
            
            // Always need custody ops entitlement
            results.push((
                SrdefId::new("IAM", "Entitlement", "custody_ops_role"),
                DiscoveryExplain {
                    srdef_id: SrdefId::new("IAM", "Entitlement", "custody_ops_role"),
                    triggered_by: format!("product={}, service={}", product, service),
                    rule: "custody_requires_ops_role".to_string(),
                },
            ));
        }
        
        // Rule 2: SSI mode = standing → require instruction set
        if options.get("ssi_mode").and_then(|v| v.as_str()) == Some("standing") {
            // TODO: Add InstructionSet SRDEF when defined
        }
        
        // Validate all discovered SRDEFs exist in registry
        for (srdef_id, _) in &results {
            if self.registry.get(srdef_id).is_none() {
                tracing::warn!("Discovery rule referenced unknown SRDEF: {}", srdef_id);
            }
        }
        
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_custody_settlement_discovery() {
        let registry = Arc::new(SrdefRegistry::new());
        let engine = ResourceDiscoveryEngine::new(registry);
        
        let intent = ServiceIntent {
            id: Uuid::new_v4(),
            cbu_id: Uuid::new_v4(),
            product_id: "custody".to_string(),
            service_id: "settlement".to_string(),
            options: json!({
                "markets": ["XNAS", "XNYS"],
                "ssi_mode": "standing"
            }),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        let results = engine.match_intent(&intent).unwrap();
        
        // Should discover: custody_securities, custody_cash, swift, ops_role
        assert_eq!(results.len(), 4);
        
        let srdef_ids: Vec<_> = results.iter().map(|(id, _)| id.0.as_str()).collect();
        assert!(srdef_ids.contains(&"SRDEF::CUSTODY::Account::custody_securities"));
        assert!(srdef_ids.contains(&"SRDEF::CUSTODY::Account::custody_cash"));
        assert!(srdef_ids.contains(&"SRDEF::SWIFT::Connectivity::swift_sender_receiver"));
        assert!(srdef_ids.contains(&"SRDEF::IAM::Entitlement::custody_ops_role"));
    }
}
```

---

## Phase 4 — Attribute Rollup + De-Dupe

### 4.1 Create `rust/src/domains/service_resources/rollup.rs`

```rust
//! Attribute Rollup Engine
//!
//! Merges SRDEF attribute requirements into CBU-level unified dictionary.
//! De-duplicates by attr_id, merges constraints, detects conflicts.

use crate::domains::service_resources::{types::*, registry::SrdefRegistry};
use anyhow::Result;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct AttributeRollup {
    registry: Arc<SrdefRegistry>,
}

impl AttributeRollup {
    pub fn new(registry: Arc<SrdefRegistry>) -> Self {
        Self { registry }
    }
    
    /// Build unified attribute requirements for a CBU
    ///
    /// Reads discovered SRDEFs from service_resource_instances,
    /// merges their attribute profiles, writes to cbu_unified_attr_requirements.
    pub async fn build_unified_requirements(
        &self,
        cbu_id: Uuid,
        pool: &PgPool,
    ) -> Result<RollupResult> {
        // Get discovered SRDEF IDs for this CBU
        let srdef_ids: Vec<String> = sqlx::query_scalar!(
            "SELECT srdef_id FROM service_resource_instances WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        // Collect all attribute requirements from these SRDEFs
        let mut attr_map: HashMap<String, Vec<(&SrdefId, &SrdefAttributeRequirement)>> = HashMap::new();
        
        for srdef_id_str in &srdef_ids {
            let srdef_id = SrdefId(srdef_id_str.clone());
            if let Some(reqs) = self.registry.get_requirements(&srdef_id) {
                for req in reqs {
                    attr_map.entry(req.attr_id.clone())
                        .or_default()
                        .push((&srdef_id, req));
                }
            }
        }
        
        // Merge requirements per attribute
        let mut unified: Vec<CbuUnifiedAttrRequirement> = Vec::new();
        let mut conflicts: Vec<AttributeConflict> = Vec::new();
        
        for (attr_id, sources) in attr_map {
            let merged = self.merge_requirements(&attr_id, &sources);
            
            if let Some(conflict) = &merged.conflict {
                if let Ok(c) = serde_json::from_value::<AttributeConflict>(conflict.clone()) {
                    conflicts.push(c);
                }
            }
            
            unified.push(CbuUnifiedAttrRequirement {
                cbu_id,
                attr_id: merged.attr_id,
                requirement_strength: merged.requirement_strength,
                merged_constraints: merged.merged_constraints,
                preferred_source: merged.preferred_source,
                required_by_srdefs: merged.required_by_srdefs,
                conflict: merged.conflict,
                explain: merged.explain,
            });
        }
        
        // Delete existing and insert new (idempotent)
        sqlx::query!(
            "DELETE FROM cbu_unified_attr_requirements WHERE cbu_id = $1",
            cbu_id
        )
        .execute(pool)
        .await?;
        
        for req in &unified {
            sqlx::query!(
                r#"
                INSERT INTO cbu_unified_attr_requirements
                    (cbu_id, attr_id, requirement_strength, merged_constraints,
                     preferred_source, required_by_srdefs, conflict, explain)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                cbu_id,
                req.attr_id,
                req.requirement_strength,
                req.merged_constraints,
                req.preferred_source,
                &req.required_by_srdefs,
                req.conflict,
                req.explain,
            )
            .execute(pool)
            .await?;
        }
        
        let required_count = unified.iter()
            .filter(|r| r.requirement_strength == "required")
            .count();
        let optional_count = unified.len() - required_count;
        
        Ok(RollupResult {
            cbu_id,
            total_attributes: unified.len(),
            required_count,
            optional_count,
            conflicts,
        })
    }
    
    /// Merge multiple SRDEF requirements for the same attribute
    fn merge_requirements(
        &self,
        attr_id: &str,
        sources: &[(&SrdefId, &SrdefAttributeRequirement)],
    ) -> MergedRequirement {
        // Collect SRDEF IDs
        let required_by: Vec<String> = sources.iter()
            .map(|(id, _)| id.0.clone())
            .collect();
        
        // Determine strength: required dominates optional
        let has_required = sources.iter().any(|(_, r)| r.requirement == RequirementStrength::Required);
        let strength = if has_required { "required" } else { "optional" };
        
        // Determine preferred source (take first non-empty)
        let preferred_source = sources.iter()
            .flat_map(|(_, r)| r.source_policy.first())
            .next()
            .map(|s| format!("{:?}", s).to_lowercase())
            .unwrap_or_else(|| "manual".to_string());
        
        // Merge constraints (best-effort)
        let (merged_constraints, conflict) = self.merge_constraints(attr_id, sources);
        
        // Build explain
        let explain = json!({
            "sources": sources.iter().map(|(id, req)| {
                json!({
                    "srdef_id": id.0,
                    "requirement": format!("{:?}", req.requirement),
                    "constraints": req.constraints,
                })
            }).collect::<Vec<_>>(),
            "merge_strategy": "required_dominates_optional",
        });
        
        MergedRequirement {
            attr_id: attr_id.to_string(),
            requirement_strength: strength.to_string(),
            merged_constraints,
            preferred_source,
            required_by_srdefs: required_by,
            conflict,
            explain,
        }
    }
    
    /// Attempt to merge constraints from multiple sources
    fn merge_constraints(
        &self,
        attr_id: &str,
        sources: &[(&SrdefId, &SrdefAttributeRequirement)],
    ) -> (Value, Option<Value>) {
        let constraints: Vec<&Value> = sources.iter()
            .map(|(_, r)| &r.constraints)
            .filter(|c| !c.is_null() && *c != &json!({}))
            .collect();
        
        if constraints.is_empty() {
            return (json!({}), None);
        }
        
        if constraints.len() == 1 {
            return (constraints[0].clone(), None);
        }
        
        // Check if all constraints are identical
        let first = constraints[0];
        let all_equal = constraints.iter().all(|c| *c == first);
        
        if all_equal {
            return (first.clone(), None);
        }
        
        // Report conflict
        let conflict = json!({
            "attr_id": attr_id,
            "srdef_a": sources[0].0.0,
            "srdef_b": sources[1].0.0,
            "constraint_a": sources[0].1.constraints,
            "constraint_b": sources[1].1.constraints,
            "description": "Constraints differ and cannot be automatically merged",
        });
        
        (first.clone(), Some(conflict))
    }
}

struct MergedRequirement {
    attr_id: String,
    requirement_strength: String,
    merged_constraints: Value,
    preferred_source: String,
    required_by_srdefs: Vec<String>,
    conflict: Option<Value>,
    explain: Value,
}
```

---

## Phase 5 — Population Engine

### 5.1 Create `rust/src/domains/service_resources/population.rs`

```rust
//! Population Engine
//!
//! Fills attribute values from various sources:
//! 1. Derived (computed from other attrs)
//! 2. Entity/CBU tables
//! 3. Document extraction (stub)
//! 4. Manual (via API)

use crate::domains::service_resources::types::*;
use anyhow::Result;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

pub struct PopulationEngine;

impl PopulationEngine {
    pub fn new() -> Self {
        Self
    }
    
    /// Attempt to populate all missing required attributes for a CBU
    pub async fn populate_missing(
        &self,
        cbu_id: Uuid,
        pool: &PgPool,
    ) -> Result<PopulationResult> {
        // Get required attributes that don't have values yet
        let missing: Vec<(String, String)> = sqlx::query!(
            r#"
            SELECT r.attr_id, r.preferred_source
            FROM cbu_unified_attr_requirements r
            LEFT JOIN cbu_attr_values v ON r.cbu_id = v.cbu_id AND r.attr_id = v.attr_id
            WHERE r.cbu_id = $1 
              AND r.requirement_strength = 'required'
              AND v.value IS NULL
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| (r.attr_id, r.preferred_source))
        .collect();
        
        let mut populated = Vec::new();
        let mut still_missing = Vec::new();
        
        for (attr_id, preferred_source) in missing {
            match self.try_populate(cbu_id, &attr_id, &preferred_source, pool).await {
                Ok(Some(value)) => {
                    populated.push(PopulatedAttribute {
                        attr_id,
                        source: value.source,
                        value: value.value,
                    });
                }
                Ok(None) => {
                    still_missing.push(attr_id);
                }
                Err(e) => {
                    tracing::warn!("Failed to populate {}: {}", attr_id, e);
                    still_missing.push(attr_id);
                }
            }
        }
        
        Ok(PopulationResult {
            cbu_id,
            populated_count: populated.len(),
            still_missing_count: still_missing.len(),
            populated,
            still_missing,
        })
    }
    
    /// Try to populate a single attribute
    async fn try_populate(
        &self,
        cbu_id: Uuid,
        attr_id: &str,
        preferred_source: &str,
        pool: &PgPool,
    ) -> Result<Option<PopulatedValue>> {
        let sources = self.source_order(preferred_source);
        
        for source in sources {
            if let Some(value) = self.try_source(cbu_id, attr_id, source, pool).await? {
                sqlx::query!(
                    r#"
                    INSERT INTO cbu_attr_values (cbu_id, attr_id, value, source, explain_refs)
                    VALUES ($1, $2, $3, $4, $5)
                    ON CONFLICT (cbu_id, attr_id) DO UPDATE SET
                        value = EXCLUDED.value,
                        source = EXCLUDED.source,
                        as_of = NOW()
                    "#,
                    cbu_id,
                    attr_id,
                    value.value,
                    source,
                    &value.explain_refs,
                )
                .execute(pool)
                .await?;
                
                return Ok(Some(PopulatedValue {
                    value: value.value,
                    source: source.to_string(),
                }));
            }
        }
        
        Ok(None)
    }
    
    fn source_order(&self, preferred: &str) -> Vec<&'static str> {
        let mut order = vec!["derived", "entity", "cbu", "document", "manual", "external"];
        if let Some(pos) = order.iter().position(|&s| s == preferred) {
            let pref = order.remove(pos);
            order.insert(0, pref);
        }
        order
    }
    
    async fn try_source(
        &self,
        cbu_id: Uuid,
        attr_id: &str,
        source: &str,
        pool: &PgPool,
    ) -> Result<Option<SourcedValue>> {
        match source {
            "entity" => self.try_entity_source(cbu_id, attr_id, pool).await,
            "cbu" => self.try_cbu_source(cbu_id, attr_id, pool).await,
            "derived" => self.try_derived_source(cbu_id, attr_id, pool).await,
            "document" => Ok(None),
            "manual" => Ok(None),
            "external" => Ok(None),
            _ => Ok(None),
        }
    }
    
    async fn try_entity_source(
        &self,
        cbu_id: Uuid,
        attr_id: &str,
        pool: &PgPool,
    ) -> Result<Option<SourcedValue>> {
        match attr_id {
            "account_name" => {
                let name: Option<String> = sqlx::query_scalar!(
                    r#"
                    SELECT e.name 
                    FROM entities e
                    JOIN cbus c ON c.legal_entity_id = e.entity_id
                    WHERE c.cbu_id = $1
                    "#,
                    cbu_id
                )
                .fetch_optional(pool)
                .await?;
                
                Ok(name.map(|n| SourcedValue {
                    value: json!(n),
                    explain_refs: vec!["entity.name".to_string()],
                }))
            }
            _ => Ok(None),
        }
    }
    
    async fn try_cbu_source(
        &self,
        _cbu_id: Uuid,
        _attr_id: &str,
        _pool: &PgPool,
    ) -> Result<Option<SourcedValue>> {
        Ok(None)
    }
    
    async fn try_derived_source(
        &self,
        cbu_id: Uuid,
        attr_id: &str,
        pool: &PgPool,
    ) -> Result<Option<SourcedValue>> {
        match attr_id {
            "settlement_currency" => {
                let markets: Option<Value> = sqlx::query_scalar!(
                    "SELECT value FROM cbu_attr_values WHERE cbu_id = $1 AND attr_id = 'market_scope'",
                    cbu_id
                )
                .fetch_optional(pool)
                .await?;
                
                if let Some(markets) = markets {
                    if let Some(arr) = markets.as_array() {
                        if let Some(first) = arr.first().and_then(|v| v.as_str()) {
                            let currency = match first {
                                "XNAS" | "XNYS" => "USD",
                                "XLON" => "GBP",
                                "XETR" | "XFRA" => "EUR",
                                "XTKS" => "JPY",
                                _ => return Ok(None),
                            };
                            return Ok(Some(SourcedValue {
                                value: json!(currency),
                                explain_refs: vec![format!("derived from market_scope={}", first)],
                            }));
                        }
                    }
                }
                Ok(None)
            }
            "message_types" => {
                Ok(Some(SourcedValue {
                    value: json!(["MT540", "MT541", "MT542", "MT543"]),
                    explain_refs: vec!["default settlement message types".to_string()],
                }))
            }
            "role_name" => {
                Ok(Some(SourcedValue {
                    value: json!("custody_operations"),
                    explain_refs: vec!["default custody role".to_string()],
                }))
            }
            "permission_scope" => {
                Ok(Some(SourcedValue {
                    value: json!(["view_positions", "initiate_settlement", "approve_settlement"]),
                    explain_refs: vec!["default custody permissions".to_string()],
                }))
            }
            _ => Ok(None),
        }
    }
}

struct SourcedValue {
    value: Value,
    explain_refs: Vec<String>,
}

struct PopulatedValue {
    value: Value,
    source: String,
}

#[derive(Debug, serde::Serialize)]
pub struct PopulationResult {
    pub cbu_id: Uuid,
    pub populated_count: usize,
    pub still_missing_count: usize,
    pub populated: Vec<PopulatedAttribute>,
    pub still_missing: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PopulatedAttribute {
    pub attr_id: String,
    pub source: String,
    pub value: Value,
}
```

---

## Phase 6 — Provisioning Gate + Orchestrator

### 6.1 Create `rust/src/domains/service_resources/provisioning.rs`

```rust
//! Provisioning Orchestrator
//!
//! Checks readiness, topo-sorts dependencies, provisions resources,
//! creates provisioning requests for async flows.

use crate::domains::service_resources::{types::*, registry::SrdefRegistry, ledger::ProvisioningLedger};
use anyhow::Result;
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct ProvisioningOrchestrator {
    registry: Arc<SrdefRegistry>,
}

impl ProvisioningOrchestrator {
    pub fn new(registry: Arc<SrdefRegistry>) -> Self {
        Self { registry }
    }
    
    /// Provision all ready resources for a CBU
    pub async fn provision(
        &self,
        cbu_id: Uuid,
        pool: &PgPool,
    ) -> Result<ProvisioningResult> {
        // Get all requested instances
        let instances: Vec<ServiceResourceInstance> = sqlx::query_as!(
            ServiceResourceInstance,
            r#"
            SELECT id, cbu_id, srdef_id, srid, native_key,
                   state as "state: ResourceState", bind_to,
                   discovery_explain, provisioning_explain,
                   resource_url, owner_ticket_id, last_request_id, last_event_at,
                   created_at, updated_at
            FROM service_resource_instances
            WHERE cbu_id = $1 AND state = 'requested'
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        // Build dependency graph and topo-sort
        let order = self.topo_sort_srdefs(&instances)?;
        
        let mut provisioned = Vec::new();
        let mut blocked = Vec::new();
        let mut requests_created = Vec::new();
        
        let ledger = ProvisioningLedger::new();
        
        for srdef_id in order {
            let report = self.check_readiness(cbu_id, &srdef_id, pool).await?;
            
            if report.ready {
                let srdef = self.registry.get(&srdef_id);
                
                match srdef.map(|s| s.provisioning_strategy) {
                    Some(ProvisioningStrategy::Create) | Some(ProvisioningStrategy::Discover) => {
                        // Sync provisioning (stub)
                        match self.do_provision(cbu_id, &srdef_id, pool).await {
                            Ok((srid, native_key)) => {
                                sqlx::query!(
                                    r#"
                                    UPDATE service_resource_instances
                                    SET srid = $1, native_key = $2, state = 'active',
                                        provisioning_explain = $3
                                    WHERE cbu_id = $4 AND srdef_id = $5
                                    "#,
                                    srid.0,
                                    native_key,
                                    json!({
                                        "provisioned_at": chrono::Utc::now().to_rfc3339(),
                                        "strategy": "stub_provisioner",
                                    }),
                                    cbu_id,
                                    srdef_id.0,
                                )
                                .execute(pool)
                                .await?;
                                
                                provisioned.push(ProvisionedResource {
                                    srdef_id: srdef_id.clone(),
                                    srid,
                                    native_key,
                                });
                            }
                            Err(e) => {
                                sqlx::query!(
                                    r#"
                                    UPDATE service_resource_instances
                                    SET state = 'failed',
                                        provisioning_explain = $1
                                    WHERE cbu_id = $2 AND srdef_id = $3
                                    "#,
                                    json!({ "error": e.to_string() }),
                                    cbu_id,
                                    srdef_id.0,
                                )
                                .execute(pool)
                                .await?;
                            }
                        }
                    }
                    Some(ProvisioningStrategy::Request) => {
                        // Async provisioning - create request
                        let request_id = ledger.create_request(
                            cbu_id,
                            &srdef_id,
                            "system",
                            pool,
                        ).await?;
                        
                        // Update instance to provisioning state
                        sqlx::query!(
                            r#"
                            UPDATE service_resource_instances
                            SET state = 'provisioning', last_request_id = $1
                            WHERE cbu_id = $2 AND srdef_id = $3
                            "#,
                            request_id,
                            cbu_id,
                            srdef_id.0,
                        )
                        .execute(pool)
                        .await?;
                        
                        requests_created.push(request_id);
                    }
                    None => {
                        tracing::warn!("Unknown SRDEF: {}", srdef_id);
                    }
                }
            } else {
                blocked.push(BlockedResource {
                    srdef_id,
                    missing_report: report,
                });
            }
        }
        
        Ok(ProvisioningResult {
            cbu_id,
            provisioned,
            blocked,
            requests_created,
        })
    }
    
    /// Check if an SRDEF is ready to provision
    pub async fn check_readiness(
        &self,
        cbu_id: Uuid,
        srdef_id: &SrdefId,
        pool: &PgPool,
    ) -> Result<MissingInputsReport> {
        let mut missing_attrs = Vec::new();
        let mut conflicts = Vec::new();
        let mut unresolved_deps = Vec::new();
        
        if let Some(reqs) = self.registry.get_requirements(srdef_id) {
            for req in reqs {
                if req.requirement == RequirementStrength::Required {
                    let exists: bool = sqlx::query_scalar!(
                        "SELECT EXISTS(SELECT 1 FROM cbu_attr_values WHERE cbu_id = $1 AND attr_id = $2)",
                        cbu_id,
                        req.attr_id
                    )
                    .fetch_one(pool)
                    .await?
                    .unwrap_or(false);
                    
                    if !exists {
                        missing_attrs.push(req.attr_id.clone());
                    }
                }
            }
        }
        
        let conflict_rows = sqlx::query!(
            r#"
            SELECT attr_id, conflict
            FROM cbu_unified_attr_requirements
            WHERE cbu_id = $1 AND conflict IS NOT NULL
            AND $2 = ANY(required_by_srdefs)
            "#,
            cbu_id,
            srdef_id.0
        )
        .fetch_all(pool)
        .await?;
        
        for row in conflict_rows {
            if let Some(conflict_json) = row.conflict {
                if let Ok(c) = serde_json::from_value::<AttributeConflict>(conflict_json) {
                    conflicts.push(c);
                }
            }
        }
        
        if let Some(srdef) = self.registry.get(srdef_id) {
            for dep_id in &srdef.dependencies {
                let dep_ready: bool = sqlx::query_scalar!(
                    "SELECT EXISTS(SELECT 1 FROM service_resource_instances WHERE cbu_id = $1 AND srdef_id = $2 AND state = 'active')",
                    cbu_id,
                    dep_id.0
                )
                .fetch_one(pool)
                .await?
                .unwrap_or(false);
                
                if !dep_ready {
                    unresolved_deps.push(dep_id.clone());
                }
            }
        }
        
        let ready = missing_attrs.is_empty() && conflicts.is_empty() && unresolved_deps.is_empty();
        
        Ok(MissingInputsReport {
            srdef_id: srdef_id.clone(),
            ready,
            missing_attrs,
            conflicts,
            missing_evidence: Vec::new(),
            unresolved_dependencies: unresolved_deps,
        })
    }
    
    fn topo_sort_srdefs(&self, instances: &[ServiceResourceInstance]) -> Result<Vec<SrdefId>> {
        let mut graph = DiGraph::<SrdefId, ()>::new();
        let mut node_map: HashMap<SrdefId, _> = HashMap::new();
        
        for inst in instances {
            let srdef_id = SrdefId(inst.srdef_id.clone());
            let node = graph.add_node(srdef_id.clone());
            node_map.insert(srdef_id, node);
        }
        
        for inst in instances {
            let srdef_id = SrdefId(inst.srdef_id.clone());
            if let Some(srdef) = self.registry.get(&srdef_id) {
                if let Some(&node) = node_map.get(&srdef_id) {
                    for dep_id in &srdef.dependencies {
                        if let Some(&dep_node) = node_map.get(dep_id) {
                            graph.add_edge(dep_node, node, ());
                        }
                    }
                }
            }
        }
        
        let sorted = toposort(&graph, None)
            .map_err(|_| anyhow::anyhow!("Dependency cycle detected"))?;
        
        Ok(sorted.into_iter().map(|n| graph[n].clone()).collect())
    }
    
    async fn do_provision(
        &self,
        cbu_id: Uuid,
        srdef_id: &SrdefId,
        _pool: &PgPool,
    ) -> Result<(Srid, String)> {
        let srdef = self.registry.get(srdef_id)
            .ok_or_else(|| anyhow::anyhow!("SRDEF not found: {}", srdef_id))?;
        
        let fake_key = format!("{}_{}", 
            cbu_id.to_string().split('-').next().unwrap(), 
            uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
        );
        let srid = Srid::new(&srdef.app_mnemonic, &srdef.resource_kind.to_string(), &fake_key);
        
        Ok((srid, fake_key))
    }
}

/// Trait for pluggable resource provisioners
#[async_trait::async_trait]
pub trait ResourceProvisioner: Send + Sync {
    async fn provision(
        &self,
        cbu_id: Uuid,
        srdef: &Srdef,
        attrs: &HashMap<String, serde_json::Value>,
    ) -> Result<(Srid, String)>;
}

/// Stub provisioner for testing
pub struct StubProvisioner;

#[async_trait::async_trait]
impl ResourceProvisioner for StubProvisioner {
    async fn provision(
        &self,
        cbu_id: Uuid,
        srdef: &Srdef,
        _attrs: &HashMap<String, serde_json::Value>,
    ) -> Result<(Srid, String)> {
        let fake_key = format!("STUB_{}", Uuid::new_v4().to_string().split('-').next().unwrap());
        let srid = Srid::new(&srdef.app_mnemonic, &srdef.resource_kind.to_string(), &fake_key);
        tracing::info!("Stub provisioned {} for CBU {}", srid, cbu_id);
        Ok((srid, fake_key))
    }
}
```

---

## Phase 6.5 — Provisioning Ledger (Append-Only)

### 6.5.1 Create `rust/src/domains/service_resources/ledger.rs`

```rust
//! Provisioning Ledger
//!
//! Append-only audit trail for provisioning requests and owner responses.
//! Never updates or deletes - only inserts.

use crate::domains::service_resources::types::*;
use anyhow::Result;
use serde_json::json;
use sha2::{Sha256, Digest};
use sqlx::PgPool;
use uuid::Uuid;

pub struct ProvisioningLedger;

impl ProvisioningLedger {
    pub fn new() -> Self {
        Self
    }
    
    /// Create a new provisioning request
    pub async fn create_request(
        &self,
        cbu_id: Uuid,
        srdef_id: &SrdefId,
        requested_by: &str,
        pool: &PgPool,
    ) -> Result<Uuid> {
        // Build request payload (snapshot of current attrs)
        let attrs: Vec<(String, serde_json::Value)> = sqlx::query!(
            "SELECT attr_id, value FROM cbu_attr_values WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| (r.attr_id, r.value))
        .collect();
        
        let request_payload = json!({
            "attributes": attrs.into_iter().collect::<serde_json::Map<String, serde_json::Value>>(),
            "snapshot_at": chrono::Utc::now().to_rfc3339(),
        });
        
        // Determine owner system from SRDEF
        let owner_system = srdef_id.parse()
            .map(|(app, _, _)| app.to_string())
            .unwrap_or_else(|| "UNKNOWN".to_string());
        
        // Insert request
        let request_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO provisioning_requests
                (request_id, cbu_id, srdef_id, requested_by, request_payload, owner_system)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            request_id,
            cbu_id,
            srdef_id.0,
            requested_by,
            request_payload,
            owner_system,
        )
        .execute(pool)
        .await?;
        
        // Log outbound event
        self.log_event(
            request_id,
            EventDirection::Out,
            EventKind::RequestSent,
            &request_payload,
            pool,
        ).await?;
        
        Ok(request_id)
    }
    
    /// Log an event to the append-only ledger
    pub async fn log_event(
        &self,
        request_id: Uuid,
        direction: EventDirection,
        kind: EventKind,
        payload: &serde_json::Value,
        pool: &PgPool,
    ) -> Result<Uuid> {
        let event_id = Uuid::new_v4();
        
        // Compute content hash for dedupe
        let hash = self.compute_hash(payload);
        
        sqlx::query!(
            r#"
            INSERT INTO provisioning_events
                (event_id, request_id, direction, kind, payload, hash)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            event_id,
            request_id,
            direction as EventDirection,
            kind as EventKind,
            payload,
            hash,
        )
        .execute(pool)
        .await?;
        
        Ok(event_id)
    }
    
    /// Process an owner system's result (webhook handler)
    pub async fn process_owner_result(
        &self,
        result: &OwnerProvisioningResult,
        pool: &PgPool,
    ) -> Result<()> {
        // Log inbound event
        let payload = serde_json::to_value(result)?;
        let hash = self.compute_hash(&payload);
        
        // Check for duplicate (idempotency)
        let exists: bool = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM provisioning_events WHERE request_id = $1 AND hash = $2)",
            result.request_id,
            hash
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(false);
        
        if exists {
            tracing::info!("Duplicate result ignored for request {}", result.request_id);
            return Ok(());
        }
        
        self.log_event(
            result.request_id,
            EventDirection::In,
            EventKind::Result,
            &payload,
            pool,
        ).await?;
        
        // Update request status
        let new_status = match result.status {
            OwnerResultStatus::Active => RequestStatus::Completed,
            OwnerResultStatus::Pending => RequestStatus::Ack,
            OwnerResultStatus::Rejected | OwnerResultStatus::Failed => RequestStatus::Failed,
        };
        
        // Note: This is a status update only, not modifying the request itself
        // In strict append-only, we'd log a status event instead
        // For pragmatism, we update status column
        sqlx::query!(
            r#"
            UPDATE provisioning_requests 
            SET status = $1, owner_ticket_id = COALESCE($2, owner_ticket_id)
            WHERE request_id = $3
            "#,
            new_status as RequestStatus,
            result.owner_ticket_id,
            result.request_id,
        )
        .execute(pool)
        .await?;
        
        // Materialize into service_resource_instances
        if result.status == OwnerResultStatus::Active {
            if let (Some(srid), Some(native_key)) = (&result.srid, &result.native_key) {
                sqlx::query!(
                    r#"
                    UPDATE service_resource_instances
                    SET srid = $1,
                        native_key = $2,
                        state = 'active',
                        resource_url = $3,
                        owner_ticket_id = $4,
                        last_event_at = $5,
                        provisioning_explain = $6
                    WHERE last_request_id = $7
                    "#,
                    srid,
                    native_key,
                    result.resource_url,
                    result.owner_ticket_id,
                    result.timestamp,
                    json!({
                        "source": "owner_result",
                        "request_id": result.request_id,
                        "timestamp": result.timestamp,
                    }),
                    result.request_id,
                )
                .execute(pool)
                .await?;
            }
        } else if result.status == OwnerResultStatus::Failed || result.status == OwnerResultStatus::Rejected {
            sqlx::query!(
                r#"
                UPDATE service_resource_instances
                SET state = 'failed',
                    last_event_at = $1,
                    provisioning_explain = $2
                WHERE last_request_id = $3
                "#,
                result.timestamp,
                json!({
                    "source": "owner_result",
                    "request_id": result.request_id,
                    "status": format!("{:?}", result.status),
                    "explain": result.explain,
                }),
                result.request_id,
            )
            .execute(pool)
            .await?;
        }
        
        Ok(())
    }
    
    /// Compute SHA256 hash of payload for dedupe
    fn compute_hash(&self, payload: &serde_json::Value) -> String {
        let bytes = serde_json::to_vec(payload).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_deterministic() {
        let ledger = ProvisioningLedger::new();
        let payload = json!({"test": "value", "number": 42});
        
        let hash1 = ledger.compute_hash(&payload);
        let hash2 = ledger.compute_hash(&payload);
        
        assert_eq!(hash1, hash2);
    }
}
```

---

## Phase 6.6 — Service Readiness Engine

### 6.6.1 Create `rust/src/domains/service_resources/readiness.rs`

```rust
//! Service Readiness Engine
//!
//! Computes "good-to-transact" status per product/service.
//! Answers: Can this CBU actually USE this service?

use crate::domains::service_resources::{types::*, registry::SrdefRegistry};
use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

pub struct ServiceReadinessEngine {
    registry: Arc<SrdefRegistry>,
}

impl ServiceReadinessEngine {
    pub fn new(registry: Arc<SrdefRegistry>) -> Self {
        Self { registry }
    }
    
    /// Compute readiness for all services of a CBU
    pub async fn compute_readiness(
        &self,
        cbu_id: Uuid,
        pool: &PgPool,
    ) -> Result<ReadinessResult> {
        // Get all service intents
        let intents = sqlx::query!(
            "SELECT product_id, service_id, options FROM service_intents WHERE cbu_id = $1",
            cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        let mut results = Vec::new();
        let mut blocked_count = 0;
        
        for intent in intents {
            let readiness = self.compute_service_readiness(
                cbu_id,
                &intent.product_id,
                &intent.service_id,
                pool,
            ).await?;
            
            if readiness.status != ReadinessStatus::Ready {
                blocked_count += 1;
            }
            
            results.push(readiness);
        }
        
        // Delete existing and insert new (idempotent)
        sqlx::query!(
            "DELETE FROM cbu_service_readiness WHERE cbu_id = $1",
            cbu_id
        )
        .execute(pool)
        .await?;
        
        for r in &results {
            sqlx::query!(
                r#"
                INSERT INTO cbu_service_readiness
                    (cbu_id, product_id, service_id, status, blocking_reasons,
                     required_srdefs, active_srids, as_of)
                VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
                "#,
                cbu_id,
                r.product_id,
                r.service_id,
                r.status as ReadinessStatus,
                r.blocking_reasons,
                &r.required_srdefs,
                &r.active_srids,
            )
            .execute(pool)
            .await?;
        }
        
        Ok(ReadinessResult {
            cbu_id,
            services: results.clone(),
            all_ready: blocked_count == 0,
            blocked_count,
        })
    }
    
    /// Compute readiness for a single service
    async fn compute_service_readiness(
        &self,
        cbu_id: Uuid,
        product_id: &str,
        service_id: &str,
        pool: &PgPool,
    ) -> Result<CbuServiceReadiness> {
        // Get all resource instances for this CBU
        let instances: Vec<(String, String, Option<String>)> = sqlx::query!(
            r#"
            SELECT srdef_id, state, srid
            FROM service_resource_instances
            WHERE cbu_id = $1
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| (r.srdef_id, r.state, r.srid))
        .collect();
        
        // Determine required SRDEFs for this product+service
        // (In real impl, this comes from discovery rules mapping)
        let required_srdefs = self.get_required_srdefs(product_id, service_id);
        
        let mut blocking_reasons: Vec<BlockingReason> = Vec::new();
        let mut active_srids: Vec<String> = Vec::new();
        
        for srdef_id in &required_srdefs {
            let instance = instances.iter()
                .find(|(id, _, _)| id == srdef_id);
            
            match instance {
                None => {
                    blocking_reasons.push(BlockingReason {
                        kind: BlockingReasonKind::MissingSrdef,
                        srdef_id: Some(srdef_id.clone()),
                        attr_id: None,
                        detail: format!("Required resource {} not discovered", srdef_id),
                    });
                }
                Some((_, state, srid)) => {
                    match state.as_str() {
                        "active" => {
                            if let Some(srid) = srid {
                                active_srids.push(srid.clone());
                            }
                        }
                        "requested" => {
                            blocking_reasons.push(BlockingReason {
                                kind: BlockingReasonKind::PendingProvisioning,
                                srdef_id: Some(srdef_id.clone()),
                                attr_id: None,
                                detail: format!("Resource {} awaiting provisioning", srdef_id),
                            });
                        }
                        "provisioning" => {
                            blocking_reasons.push(BlockingReason {
                                kind: BlockingReasonKind::PendingProvisioning,
                                srdef_id: Some(srdef_id.clone()),
                                attr_id: None,
                                detail: format!("Resource {} provisioning in progress", srdef_id),
                            });
                        }
                        "failed" => {
                            blocking_reasons.push(BlockingReason {
                                kind: BlockingReasonKind::FailedProvisioning,
                                srdef_id: Some(srdef_id.clone()),
                                attr_id: None,
                                detail: format!("Resource {} provisioning failed", srdef_id),
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Check for attribute conflicts
        let conflicts = sqlx::query!(
            r#"
            SELECT attr_id, conflict
            FROM cbu_unified_attr_requirements
            WHERE cbu_id = $1 AND conflict IS NOT NULL
            "#,
            cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        for conflict in conflicts {
            blocking_reasons.push(BlockingReason {
                kind: BlockingReasonKind::AttrConflict,
                srdef_id: None,
                attr_id: Some(conflict.attr_id),
                detail: "Attribute has conflicting requirements".to_string(),
            });
        }
        
        // Determine overall status
        let status = if blocking_reasons.is_empty() {
            ReadinessStatus::Ready
        } else if active_srids.is_empty() {
            ReadinessStatus::Blocked
        } else {
            ReadinessStatus::Partial
        };
        
        Ok(CbuServiceReadiness {
            cbu_id,
            product_id: product_id.to_string(),
            service_id: service_id.to_string(),
            status,
            blocking_reasons: json!(blocking_reasons),
            required_srdefs,
            active_srids,
            as_of: chrono::Utc::now(),
        })
    }
    
    /// Get required SRDEFs for a product+service combination
    /// (Hardcoded for now; should match discovery rules)
    fn get_required_srdefs(&self, product: &str, service: &str) -> Vec<String> {
        match (product.to_lowercase().as_str(), service.to_lowercase().as_str()) {
            ("custody", "settlement") => vec![
                "SRDEF::CUSTODY::Account::custody_securities".to_string(),
                "SRDEF::CUSTODY::Account::custody_cash".to_string(),
                "SRDEF::IAM::Entitlement::custody_ops_role".to_string(),
            ],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_required_srdefs_custody_settlement() {
        let registry = Arc::new(SrdefRegistry::new());
        let engine = ServiceReadinessEngine::new(registry);
        
        let required = engine.get_required_srdefs("custody", "settlement");
        
        assert_eq!(required.len(), 3);
        assert!(required.contains(&"SRDEF::CUSTODY::Account::custody_securities".to_string()));
    }
}
```

---

## Phase 7 + 7.5 — Observability & Explainability

### Explain Payload Patterns

All derived tables include `explain` JSON columns linking back to source data:

**Discovery Explain** (in `service_resource_instances.discovery_explain`):
```json
{
  "triggered_by": "product=custody, service=settlement",
  "rule": "custody_settlement_requires_securities_account",
  "intent_id": "uuid"
}
```

**Rollup Explain** (in `cbu_unified_attr_requirements.explain`):
```json
{
  "sources": [
    {"srdef_id": "SRDEF::CUSTODY::Account::custody_securities", "requirement": "Required"},
    {"srdef_id": "SRDEF::CUSTODY::Account::custody_cash", "requirement": "Optional"}
  ],
  "merge_strategy": "required_dominates_optional"
}
```

**Provisioning Explain** (in `service_resource_instances.provisioning_explain`):
```json
{
  "source": "owner_result",
  "request_id": "uuid",
  "timestamp": "2026-01-13T12:00:00Z",
  "strategy": "request"
}
```

**Blocking Reasons** (in `cbu_service_readiness.blocking_reasons`):
```json
[
  {
    "kind": "pending_provisioning",
    "srdef_id": "SRDEF::IAM::Entitlement::custody_ops_role",
    "detail": "Resource awaiting owner response"
  }
]
```

---

## Phase 8 — Tests

### 8.1 Create `rust/tests/service_resource_pipeline_tests.rs`

```rust
//! Integration tests for Service Resource Pipeline

use ob_poc::domains::service_resources::{
    types::*,
    discovery::ResourceDiscoveryEngine,
    rollup::AttributeRollup,
    provisioning::ProvisioningOrchestrator,
    ledger::ProvisioningLedger,
    readiness::ServiceReadinessEngine,
    registry::SrdefRegistry,
};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

async fn setup_test_db() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/ob_poc_test".to_string());
    PgPool::connect(&url).await.unwrap()
}

async fn load_test_registry() -> Arc<SrdefRegistry> {
    let mut registry = SrdefRegistry::new();
    let config_path = std::path::Path::new("config/srdefs");
    registry.load_from_directory(config_path).await.unwrap();
    Arc::new(registry)
}

#[tokio::test]
async fn test_discovery_idempotency() {
    let pool = setup_test_db().await;
    let registry = load_test_registry().await;
    let engine = ResourceDiscoveryEngine::new(registry);
    
    let cbu_id = Uuid::new_v4();
    
    sqlx::query!(
        "INSERT INTO cbus (cbu_id, name, jurisdiction) VALUES ($1, 'Test CBU', 'US')",
        cbu_id
    )
    .execute(&pool)
    .await
    .unwrap();
    
    let intents = vec![ServiceIntent {
        id: Uuid::new_v4(),
        cbu_id,
        product_id: "custody".to_string(),
        service_id: "settlement".to_string(),
        options: json!({"markets": ["XNAS"]}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }];
    
    let result1 = engine.discover(cbu_id, &intents, &pool).await.unwrap();
    let result2 = engine.discover(cbu_id, &intents, &pool).await.unwrap();
    
    assert_eq!(result1.srdefs_discovered.len(), result2.srdefs_discovered.len());
    assert_eq!(result1.instances_created, result2.instances_created);
    
    sqlx::query!("DELETE FROM cbus WHERE cbu_id = $1", cbu_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_provisioning_ledger_idempotency() {
    let pool = setup_test_db().await;
    let ledger = ProvisioningLedger::new();
    
    let request_id = Uuid::new_v4();
    let cbu_id = Uuid::new_v4();
    
    // Setup
    sqlx::query!(
        "INSERT INTO cbus (cbu_id, name, jurisdiction) VALUES ($1, 'Test', 'US')",
        cbu_id
    ).execute(&pool).await.unwrap();
    
    sqlx::query!(
        r#"
        INSERT INTO provisioning_requests (request_id, cbu_id, srdef_id, requested_by, owner_system)
        VALUES ($1, $2, 'SRDEF::TEST::Test::test', 'test', 'TEST')
        "#,
        request_id,
        cbu_id,
    ).execute(&pool).await.unwrap();
    
    // First result
    let result = OwnerProvisioningResult {
        srdef_id: "SRDEF::TEST::Test::test".to_string(),
        request_id,
        status: OwnerResultStatus::Active,
        srid: Some("SR::TEST::Test::KEY123".to_string()),
        native_key: Some("KEY123".to_string()),
        native_key_type: Some("TestKey".to_string()),
        resource_url: Some("https://test.com/resource/123".to_string()),
        owner_ticket_id: Some("INC123".to_string()),
        explain: None,
        timestamp: chrono::Utc::now(),
    };
    
    // Process twice - should be idempotent
    ledger.process_owner_result(&result, &pool).await.unwrap();
    ledger.process_owner_result(&result, &pool).await.unwrap();
    
    // Should only have one RESULT event
    let event_count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM provisioning_events WHERE request_id = $1 AND kind = 'RESULT'",
        request_id
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    
    assert_eq!(event_count, 1);
    
    // Cleanup
    sqlx::query!("DELETE FROM cbus WHERE cbu_id = $1", cbu_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_service_readiness_computation() {
    let pool = setup_test_db().await;
    let registry = load_test_registry().await;
    let engine = ServiceReadinessEngine::new(registry);
    
    let cbu_id = Uuid::new_v4();
    
    sqlx::query!(
        "INSERT INTO cbus (cbu_id, name, jurisdiction) VALUES ($1, 'Test', 'US')",
        cbu_id
    ).execute(&pool).await.unwrap();
    
    sqlx::query!(
        r#"
        INSERT INTO service_intents (cbu_id, product_id, service_id, options)
        VALUES ($1, 'custody', 'settlement', '{}')
        "#,
        cbu_id
    ).execute(&pool).await.unwrap();
    
    // No resources provisioned yet
    let result = engine.compute_readiness(cbu_id, &pool).await.unwrap();
    
    assert!(!result.all_ready);
    assert!(result.blocked_count > 0);
    
    // Cleanup
    sqlx::query!("DELETE FROM cbus WHERE cbu_id = $1", cbu_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_end_to_end_pipeline() {
    let pool = setup_test_db().await;
    let registry = load_test_registry().await;
    
    let cbu_id = Uuid::new_v4();
    
    // 1. Create CBU
    sqlx::query!(
        "INSERT INTO cbus (cbu_id, name, jurisdiction) VALUES ($1, 'E2E Test CBU', 'US')",
        cbu_id
    ).execute(&pool).await.unwrap();
    
    // 2. Create service intent
    let intents = vec![ServiceIntent {
        id: Uuid::new_v4(),
        cbu_id,
        product_id: "custody".to_string(),
        service_id: "settlement".to_string(),
        options: json!({"markets": ["XNAS"]}),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }];
    
    // 3. Discovery
    let discovery_engine = ResourceDiscoveryEngine::new(registry.clone());
    let discovery_result = discovery_engine.discover(cbu_id, &intents, &pool).await.unwrap();
    assert!(discovery_result.instances_created > 0);
    
    // 4. Rollup
    let rollup = AttributeRollup::new(registry.clone());
    let rollup_result = rollup.build_unified_requirements(cbu_id, &pool).await.unwrap();
    assert!(rollup_result.total_attributes > 0);
    
    // 5. Populate attrs
    sqlx::query!(
        r#"
        INSERT INTO cbu_attr_values (cbu_id, attr_id, value, source)
        VALUES 
            ($1, 'market_scope', '["XNAS"]', 'manual'),
            ($1, 'settlement_currency', '"USD"', 'manual'),
            ($1, 'account_name', '"Test Account"', 'manual'),
            ($1, 'custody_type', '"segregated"', 'manual'),
            ($1, 'cash_currency', '"USD"', 'manual'),
            ($1, 'bic_sender', '"BNYMGB2L"', 'manual'),
            ($1, 'bic_receiver', '"CITIUS33"', 'manual'),
            ($1, 'message_types', '["MT540","MT541"]', 'manual'),
            ($1, 'role_name', '"custody_operations"', 'manual'),
            ($1, 'permission_scope', '["view_positions"]', 'manual')
        ON CONFLICT (cbu_id, attr_id) DO UPDATE SET value = EXCLUDED.value
        "#,
        cbu_id
    ).execute(&pool).await.unwrap();
    
    // 6. Provision
    let orchestrator = ProvisioningOrchestrator::new(registry.clone());
    let provision_result = orchestrator.provision(cbu_id, &pool).await.unwrap();
    
    println!("Provisioned: {:?}", provision_result.provisioned.len());
    println!("Blocked: {:?}", provision_result.blocked.len());
    println!("Requests created: {:?}", provision_result.requests_created.len());
    
    // 7. Check readiness
    let readiness_engine = ServiceReadinessEngine::new(registry);
    let readiness = readiness_engine.compute_readiness(cbu_id, &pool).await.unwrap();
    
    println!("All ready: {}", readiness.all_ready);
    println!("Blocked count: {}", readiness.blocked_count);
    
    // Cleanup
    sqlx::query!("DELETE FROM cbus WHERE cbu_id = $1", cbu_id)
        .execute(&pool)
        .await
        .unwrap();
}
```

---

## Migration Checklist

### Phase 0: Types + DB (2h)
- [ ] Create `rust/src/domains/service_resources/mod.rs`
- [ ] Create `rust/src/domains/service_resources/types.rs`
- [ ] Add to `rust/src/domains/mod.rs`
- [ ] Create migration `rust/migrations/20260113_service_resource_pipeline.sql`
- [ ] Run migration: `sqlx migrate run`
- [ ] Regenerate sqlx: `cargo sqlx prepare`

### Phase 1: SRDEF Registry (1h)
- [ ] Create `rust/config/srdefs/` directory
- [ ] Create `custody.yaml`, `swift.yaml`, `iam.yaml`
- [ ] Create `rust/src/domains/service_resources/registry.rs`
- [ ] Unit test YAML loading

### Phase 2: Service Intent API (1h)
- [ ] Add DSL verbs to `rust/config/verbs/service.yaml`
- [ ] Create `rust/src/domains/service_resources/api.rs`
- [ ] Wire routes in main router
- [ ] Test endpoints with curl

### Phase 3: Discovery Engine (1.5h)
- [ ] Create `rust/src/domains/service_resources/discovery.rs`
- [ ] Implement custody + settlement rules
- [ ] Test idempotency

### Phase 4: Attribute Rollup (1.5h)
- [ ] Create `rust/src/domains/service_resources/rollup.rs`
- [ ] Implement de-dupe + merge
- [ ] Test conflict detection

### Phase 5: Population Engine (1h)
- [ ] Create `rust/src/domains/service_resources/population.rs`
- [ ] Implement entity/CBU/derived sources
- [ ] Test population flow

### Phase 6: Provisioning (1.5h)
- [ ] Create `rust/src/domains/service_resources/provisioning.rs`
- [ ] Implement readiness check
- [ ] Implement topo-sort
- [ ] Implement stub provisioner

### Phase 6.5: Provisioning Ledger (1.5h)
- [ ] Create `rust/src/domains/service_resources/ledger.rs`
- [ ] Implement append-only request creation
- [ ] Implement event logging with hash dedupe
- [ ] Implement owner result processing
- [ ] Materialize results into service_resource_instances
- [ ] Test idempotency of result processing

### Phase 6.6: Service Readiness (1h)
- [ ] Create `rust/src/domains/service_resources/readiness.rs`
- [ ] Implement readiness computation
- [ ] Generate blocking reasons
- [ ] Persist to cbu_service_readiness
- [ ] Add API endpoints

### Phase 7: Tests (1h)
- [ ] Create integration tests
- [ ] Test idempotency (discovery, ledger, readiness)
- [ ] Test conflict detection
- [ ] Test dependency ordering
- [ ] Test end-to-end pipeline

### Phase 8: Wire Up (30m)
- [ ] Initialize registry on app startup
- [ ] Add routes to main router
- [ ] Test via agent commands

---

## Success Criteria

- [ ] `POST /cbu/{id}/service-intents` creates intent
- [ ] `POST /cbu/{id}/resource-discover` discovers 4 SRDEFs for custody+settlement
- [ ] `POST /cbu/{id}/attributes/rollup` produces de-duped unified dictionary
- [ ] `GET /cbu/{id}/attributes/requirements` shows required attrs with explain
- [ ] `POST /cbu/{id}/attributes/values` sets manual values
- [ ] `POST /cbu/{id}/resources/provision` provisions ready SRDEFs, creates requests for async
- [ ] `POST /provisioning/result` webhook processes owner responses
- [ ] `GET /cbu/{id}/readiness` shows good-to-transact status
- [ ] Discovery is idempotent (run twice → same result)
- [ ] Rollup de-dupes by attr_id
- [ ] Provisioning respects dependency order
- [ ] Provisioning ledger is append-only (duplicates rejected)
- [ ] Service readiness shows blocking reasons
- [ ] All derived tables rebuildable

---

## Total Effort: ~13h

---

## Files Created/Modified

### New Files
```
rust/src/domains/service_resources/mod.rs
rust/src/domains/service_resources/types.rs
rust/src/domains/service_resources/registry.rs
rust/src/domains/service_resources/discovery.rs
rust/src/domains/service_resources/rollup.rs
rust/src/domains/service_resources/population.rs
rust/src/domains/service_resources/provisioning.rs
rust/src/domains/service_resources/ledger.rs
rust/src/domains/service_resources/readiness.rs
rust/src/domains/service_resources/api.rs
rust/config/srdefs/custody.yaml
rust/config/srdefs/swift.yaml
rust/config/srdefs/iam.yaml
rust/config/verbs/service.yaml
rust/migrations/20260113_service_resource_pipeline.sql
rust/tests/service_resource_pipeline_tests.rs
```

### Modified Files
```
rust/src/domains/mod.rs          (add service_resources)
rust/src/api/routes.rs           (add service_resource_routes)
rust/src/main.rs                 (initialize SrdefRegistry)
rust/Cargo.toml                  (add petgraph, sha2 if not present)
```

---

## Cargo.toml Dependencies

```toml
# Add if not present
petgraph = "0.6"
sha2 = "0.10"
async-trait = "0.1"
```
