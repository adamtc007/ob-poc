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
pub struct ServiceIntent {
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
pub struct NewServiceIntent {
    pub cbu_id: Uuid,
    pub product_id: Uuid,
    pub service_id: Uuid,
    pub options: Option<JsonValue>,
    pub created_by: Option<String>,
}

/// Service intent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceIntentStatus {
    Active,
    Suspended,
    Cancelled,
}

impl std::fmt::Display for ServiceIntentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

// =============================================================================
// SRDEF (ServiceResourceDefinition)
// =============================================================================

/// Service Resource Definition - defines what resources are needed.
/// Maps to service_resource_types table with srdef_id computed column.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Srdef {
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

/// How to obtain a resource
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvisioningStrategy {
    /// We create it directly
    Create,
    /// We request it from an owner system
    Request,
    /// We discover/bind to an existing resource
    Discover,
}

impl Default for ProvisioningStrategy {
    fn default() -> Self {
        Self::Create
    }
}

impl std::fmt::Display for ProvisioningStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "create"),
            Self::Request => write!(f, "request"),
            Self::Discover => write!(f, "discover"),
        }
    }
}

// =============================================================================
// SRDEF DISCOVERY
// =============================================================================

/// Why a particular SRDEF was discovered for a CBU.
/// Output of the resource discovery engine.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SrdefDiscoveryReason {
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
pub struct NewSrdefDiscovery {
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
pub struct CbuUnifiedAttrRequirement {
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

/// Requirement strength
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequirementStrength {
    Required,
    Optional,
    Conditional,
}

impl Default for RequirementStrength {
    fn default() -> Self {
        Self::Required
    }
}

impl std::fmt::Display for RequirementStrength {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Required => write!(f, "required"),
            Self::Optional => write!(f, "optional"),
            Self::Conditional => write!(f, "conditional"),
        }
    }
}

/// Attribute source (where value came from)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttributeSource {
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
pub struct CbuAttrValue {
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
pub struct SetCbuAttrValue {
    pub cbu_id: Uuid,
    pub attr_id: Uuid,
    pub value: JsonValue,
    pub source: AttributeSource,
    pub evidence_refs: Option<Vec<EvidenceRef>>,
    pub explain_refs: Option<Vec<ExplainRef>>,
}

/// Reference to evidence supporting an attribute value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    #[serde(rename = "type")]
    pub ref_type: String, // "document", "entity_field", "api_response", etc.
    pub id: Option<String>,
    pub path: Option<String>,
    pub details: Option<JsonValue>,
}

/// Explanation of how a value was derived
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainRef {
    pub rule: String,
    pub input: Option<JsonValue>,
    pub output: Option<JsonValue>,
}

// =============================================================================
// PROVISIONING REQUESTS
// =============================================================================

/// A provisioning request to an owner system.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProvisioningRequest {
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
pub struct NewProvisioningRequest {
    pub cbu_id: Uuid,
    pub srdef_id: String,
    pub instance_id: Option<Uuid>,
    pub requested_by: RequestedBy,
    pub request_payload: ProvisioningPayload,
    pub owner_system: String,
    pub parameters: Option<JsonValue>,
}

/// Who requested the provisioning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestedBy {
    Agent,
    User,
    System,
}

impl Default for RequestedBy {
    fn default() -> Self {
        Self::System
    }
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

/// Provisioning request status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvisioningStatus {
    Queued,
    Sent,
    Ack,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for ProvisioningStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Sent => write!(f, "sent"),
            Self::Ack => write!(f, "ack"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Payload for a provisioning request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningPayload {
    pub attrs: JsonValue,
    pub bind_to: Option<JsonValue>,
    pub evidence_refs: Option<Vec<EvidenceRef>>,
    pub idempotency_key: Option<String>,
}

// =============================================================================
// PROVISIONING EVENTS
// =============================================================================

/// An event in the provisioning lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProvisioningEvent {
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
pub enum EventDirection {
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
pub enum EventKind {
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

/// Result payload from owner system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerProvisioningResult {
    pub status: String, // "active", "pending", "rejected", "failed"
    pub srid: Option<String>,
    pub native_key: Option<String>,
    pub native_key_type: Option<String>,
    pub resource_url: Option<String>,
    pub owner_ticket_id: Option<String>,
    pub explain: Option<ResultExplanation>,
    pub timestamp: Option<DateTime<Utc>>,
}

/// Explanation for result (especially failures)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultExplanation {
    pub message: Option<String>,
    pub codes: Option<Vec<String>>,
}

// =============================================================================
// SERVICE READINESS
// =============================================================================

/// Service readiness status for a CBU.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CbuServiceReadiness {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadinessStatus {
    Ready,
    Blocked,
    Partial,
}

impl Default for ReadinessStatus {
    fn default() -> Self {
        Self::Blocked
    }
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
pub struct BlockingReason {
    #[serde(rename = "type")]
    pub reason_type: BlockingReasonType,
    pub srdef_id: Option<String>,
    pub details: JsonValue,
    pub explain: String,
}

/// Types of blocking reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockingReasonType {
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

// =============================================================================
// ATTRIBUTE GAP REPORT
// =============================================================================

/// Report of missing attributes for a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeGapReport {
    pub cbu_id: Uuid,
    pub total_required: usize,
    pub populated: usize,
    pub missing: Vec<MissingAttribute>,
    pub conflicts: Vec<AttributeConflict>,
    pub pct_complete: f64,
}

/// A missing required attribute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingAttribute {
    pub attr_id: Uuid,
    pub attr_code: String,
    pub attr_name: String,
    pub category: String,
    pub required_by_srdefs: Vec<String>,
    pub preferred_source: Option<String>,
}

/// A conflict in attribute requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeConflict {
    pub attr_id: Uuid,
    pub attr_name: String,
    pub conflicting_srdefs: Vec<String>,
    pub conflict_type: String,
    pub details: JsonValue,
}

// =============================================================================
// SRDEF READINESS CHECK
// =============================================================================

/// Result of checking if a single SRDEF is ready
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrdefReadinessCheck {
    pub srdef_id: String,
    pub is_ready: bool,
    pub missing_attrs: Vec<Uuid>,
    pub instance_status: Option<String>,
    pub blocking_reason: Option<String>,
}

// =============================================================================
// API RESPONSE TYPES
// =============================================================================

/// Response for service intent operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceIntentResponse {
    pub intent_id: Uuid,
    pub cbu_id: Uuid,
    pub product_name: String,
    pub service_name: String,
    pub options: JsonValue,
    pub status: String,
}

/// Response for discovery operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResponse {
    pub cbu_id: Uuid,
    pub discovered_srdefs: Vec<DiscoveredSrdef>,
    pub total_discovered: usize,
}

/// A discovered SRDEF with reason
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSrdef {
    pub srdef_id: String,
    pub resource_name: String,
    pub discovery_rule: String,
    pub triggered_by: Vec<String>,
    pub parameters: Option<JsonValue>,
}

/// Response for readiness query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub services: Vec<ServiceReadinessDetail>,
    pub summary: ReadinessSummary,
}

/// Detail for a single service's readiness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceReadinessDetail {
    pub product_name: String,
    pub service_name: String,
    pub status: ReadinessStatus,
    pub blocking_reasons: Vec<BlockingReason>,
    pub required_srdefs: Vec<String>,
    pub active_instances: Vec<String>,
}

/// Summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessSummary {
    pub total_services: usize,
    pub ready: usize,
    pub partial: usize,
    pub blocked: usize,
    pub pct_ready: f64,
}
