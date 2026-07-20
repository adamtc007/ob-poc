//! Service Resources Pipeline Types
//!
//! Core types for the CBU Service → Resource Discovery → Unified Dictionary → Provisioning pipeline.
//!
//! Key concepts:
//! - ServiceIntent: What a CBU wants (product + service + options)
//! - SRDEF: ServiceResourceDefinition - what resources are needed
//! - CbuUnifiedAttrRequirement: Rolled-up attribute requirements per CBU
//! - ProvisioningRequest/Event: Audit trail for provisioning
//! - ServiceReadiness: "Good to transact" status

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

// =============================================================================
// SERVICE INTENT
// =============================================================================

/// What a CBU wants: a subscription to a product/service combination with options.
/// This is the INPUT to the resource discovery engine.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct ServiceIntent {
    pub intent_id: Uuid,
    pub cbu_id: Uuid,
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub options: JsonValue,
    pub status: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
}

/// Input for creating a new service intent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewServiceIntent {
    pub cbu_id: Uuid,
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub options: Option<JsonValue>,
    pub created_by: Option<String>,
}

// =============================================================================
// SRDEF (ServiceResourceDefinition)
// =============================================================================

/// Service Resource Definition - defines what resources are needed.
/// Maps to service_resource_types table with srdef_id computed column.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct Srdef {
    pub resource_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner: String,
    pub resource_code: Option<String>,
    pub resource_type: Option<String>,
    pub resource_purpose: Option<String>,
    pub srdef_id: Option<String>,
    pub provisioning_strategy: Option<String>,
    pub depends_on: Option<JsonValue>,
    pub is_active: Option<bool>,
}

// =============================================================================
// SRDEF DISCOVERY
// =============================================================================

/// Why a particular SRDEF was discovered for a CBU.
/// Output of the resource discovery engine.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct SrdefDiscoveryReason {
    pub discovery_id: Uuid,
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub resource_type_id: Option<Uuid>,
    pub triggered_by_intents: JsonValue,
    pub discovery_rule: String,
    pub discovery_reason: JsonValue,
    pub parameters: Option<JsonValue>,
    pub discovered_at: DateTime<Utc>,
    pub superseded_at: Option<DateTime<Utc>>,
}

/// Input for recording a discovery reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewSrdefDiscovery {
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub resource_type_id: Option<Uuid>,
    pub triggered_by_intents: Vec<Uuid>,
    pub discovery_rule: String,
    pub discovery_reason: JsonValue,
    pub parameters: Option<JsonValue>,
}

// =============================================================================
// CBU UNIFIED ATTRIBUTE REQUIREMENTS
// =============================================================================

/// Rolled-up attribute requirement for a CBU.
/// Derived from all discovered SRDEFs.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct CbuUnifiedAttrRequirement {
    pub cbu_id: Uuid,
    pub attr_id: Uuid,
    pub requirement_strength: String,
    pub merged_constraints: JsonValue,
    pub preferred_source: Option<String>,
    pub required_by_srdefs: JsonValue,
    pub conflict: Option<JsonValue>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Attribute source (where value came from)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum AttributeSource {
    Derived,
    Entity,
    Cbu,
    Document,
    Manual,
    External,
}

impl std::fmt::Display for AttributeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Derived => write!(f, "derived"),
            Self::Entity => write!(f, "entity"),
            Self::Cbu => write!(f, "cbu"),
            Self::Document => write!(f, "document"),
            Self::Manual => write!(f, "manual"),
            Self::External => write!(f, "external"),
        }
    }
}

// =============================================================================
// CBU ATTRIBUTE VALUES
// =============================================================================

/// CBU-level attribute value (populated from various sources).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct CbuAttrValue {
    pub cbu_id: Uuid,
    pub attr_id: Uuid,
    pub value: JsonValue,
    pub source: String,
    pub evidence_refs: JsonValue,
    pub explain_refs: JsonValue,
    pub as_of: DateTime<Utc>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Input for setting a CBU attribute value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SetCbuAttrValue {
    pub cbu_id: Uuid,
    pub attr_id: Uuid,
    pub value: JsonValue,
    pub source: AttributeSource,
    pub evidence_refs: Option<Vec<EvidenceRef>>,
    pub explain_refs: Option<Vec<ExplainRef>>,
}

/// Reference to evidence supporting an attribute value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvidenceRef {
    #[serde(rename = "type")]
    pub ref_type: String, // "document", "entity_field", "api_response", etc.
    pub id: Option<String>,
    pub path: Option<String>,
    pub details: Option<JsonValue>,
}

/// Explanation of how a value was derived
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExplainRef {
    pub rule: String,
    pub input: Option<JsonValue>,
    pub output: Option<JsonValue>,
}

// =============================================================================
// PROVISIONING REQUESTS
// =============================================================================

/// A provisioning request to an owner system.
// kept for sqlx query_as
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[allow(dead_code)]
pub(crate) struct ProvisioningRequest {
    pub request_id: Uuid,
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub instance_id: Option<Uuid>,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
    pub request_payload: JsonValue,
    pub status: String,
    pub owner_system: String,
    pub owner_ticket_id: Option<String>,
    pub parameters: Option<JsonValue>,
    pub status_changed_at: Option<DateTime<Utc>>,
}

/// Input for creating a new provisioning request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NewProvisioningRequest {
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub instance_id: Option<Uuid>,
    pub requested_by: RequestedBy,
    pub request_payload: ProvisioningPayload,
    pub owner_system: String,
    pub parameters: Option<JsonValue>,
}

/// Who requested the provisioning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RequestedBy {
    Agent,
    User,
    #[default]
    System,
}

impl std::fmt::Display for RequestedBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent => write!(f, "agent"),
            Self::User => write!(f, "user"),
            Self::System => write!(f, "system"),
        }
    }
}

/// Payload for a provisioning request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProvisioningPayload {
    pub attrs: JsonValue,
    pub bind_to: Option<JsonValue>,
    pub evidence_refs: Option<Vec<EvidenceRef>>,
    pub idempotency_key: Option<String>,
}

// =============================================================================
// PROVISIONING EVENTS
// =============================================================================

/// An event in the provisioning lifecycle.
// kept for sqlx query_as
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[allow(dead_code)]
pub(crate) struct ProvisioningEvent {
    pub event_id: Uuid,
    pub request_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub direction: String,
    pub kind: String,
    pub payload: JsonValue,
    pub content_hash: Option<String>,
}

/// Event direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum EventDirection {
    #[serde(rename = "OUT")]
    Out,
    #[serde(rename = "IN")]
    In,
}

impl std::fmt::Display for EventDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Out => write!(f, "OUT"),
            Self::In => write!(f, "IN"),
        }
    }
}

/// Event kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum EventKind {
    RequestSent,
    Ack,
    Result,
    Error,
    Status,
    Retry,
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestSent => write!(f, "REQUEST_SENT"),
            Self::Ack => write!(f, "ACK"),
            Self::Result => write!(f, "RESULT"),
            Self::Error => write!(f, "ERROR"),
            Self::Status => write!(f, "STATUS"),
            Self::Retry => write!(f, "RETRY"),
        }
    }
}

// =============================================================================
// SERVICE READINESS
// =============================================================================

/// Service readiness status for a CBU.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub(crate) struct CbuServiceReadiness {
    pub cbu_id: Uuid,
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub status: String,
    pub blocking_reasons: JsonValue,
    pub required_srdefs: JsonValue,
    pub active_srids: JsonValue,
    pub as_of: DateTime<Utc>,
    pub last_recomputed_at: Option<DateTime<Utc>>,
    pub recomputation_trigger: Option<String>,
    pub is_stale: Option<bool>,
}

/// Readiness status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReadinessStatus {
    Ready,
    #[default]
    Blocked,
    Partial,
}

impl std::fmt::Display for ReadinessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "ready"),
            Self::Blocked => write!(f, "blocked"),
            Self::Partial => write!(f, "partial"),
        }
    }
}

/// A reason why a service is blocked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BlockingReason {
    #[serde(rename = "type")]
    pub reason_type: BlockingReasonType,
    pub srdef_id: Option<String>,
    pub details: JsonValue,
    pub explain: String,
}

/// Types of blocking reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BlockingReasonType {
    MissingSrdef,
    PendingProvisioning,
    FailedProvisioning,
    MissingAttrs,
    AttrConflict,
    DependencyNotReady,
}

impl std::fmt::Display for BlockingReasonType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSrdef => write!(f, "missing_srdef"),
            Self::PendingProvisioning => write!(f, "pending_provisioning"),
            Self::FailedProvisioning => write!(f, "failed_provisioning"),
            Self::MissingAttrs => write!(f, "missing_attrs"),
            Self::AttrConflict => write!(f, "attr_conflict"),
            Self::DependencyNotReady => write!(f, "dependency_not_ready"),
        }
    }
}
