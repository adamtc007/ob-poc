//! Booking Principal Types
//!
//! Type definitions for the Booking Principal selection capability.
//! Covers core entities (legal entity, location, principal), evaluation
//! snapshots (client profile, classifications), three-lane availability,
//! rules/rulesets, contract packs, and eligibility evaluation results.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Entity Types (Tier 1 — stable reference data)
// ============================================================================

/// BNY legal entity that can sign contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalEntity {
    pub legal_entity_id: Uuid,
    pub lei: Option<String>,
    pub name: String,
    pub incorporation_jurisdiction: String,
    pub status: String,
    pub entity_id: Option<Uuid>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Jurisdictional perimeter in which activity is booked/regulated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingLocation {
    pub booking_location_id: Uuid,
    pub country_code: String,
    pub region_code: Option<String>,
    pub regulatory_regime_tags: Vec<String>,
    pub jurisdiction_code: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Contracting + booking authority envelope: LegalEntity + BookingLocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingPrincipal {
    pub booking_principal_id: Uuid,
    pub legal_entity_id: Uuid,
    pub booking_location_id: Option<Uuid>,
    pub principal_code: String,
    pub book_code: Option<String>,
    pub status: String,
    pub effective_from: DateTime<Utc>,
    pub effective_to: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Profile & Classification Types (Tier 2 — evaluation snapshots)
// ============================================================================

/// Point-in-time evaluation snapshot of client facts (immutable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientProfile {
    pub client_profile_id: Uuid,
    pub client_group_id: Uuid,
    pub as_of: DateTime<Utc>,
    pub segment: String,
    pub domicile_country: String,
    pub entity_types: Vec<String>,
    pub risk_flags: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Normalised regulatory classification per profile snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientClassification {
    pub client_classification_id: Uuid,
    pub client_profile_id: Uuid,
    pub classification_scheme: String,
    pub classification_value: String,
    pub jurisdiction_scope: Option<String>,
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_to: Option<DateTime<Utc>>,
    pub source: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Three-lane availability per booking_principal x service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAvailabilityRecord {
    pub service_availability_id: Uuid,
    pub booking_principal_id: Uuid,
    pub service_id: Uuid,
    // Lane 1: Regulatory
    pub regulatory_status: String,
    pub regulatory_constraints: Option<serde_json::Value>,
    // Lane 2: Commercial
    pub commercial_status: String,
    pub commercial_constraints: Option<serde_json::Value>,
    // Lane 3: Operational
    pub operational_status: String,
    pub delivery_model: Option<String>,
    pub operational_constraints: Option<serde_json::Value>,
    // Temporal
    pub effective_from: DateTime<Utc>,
    pub effective_to: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Active client-principal-offering relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPrincipalRelationship {
    pub client_principal_relationship_id: Uuid,
    pub client_group_id: Uuid,
    pub booking_principal_id: Uuid,
    pub product_offering_id: Uuid,
    pub relationship_status: String,
    pub contract_ref: Option<String>,
    pub onboarded_at: Option<DateTime<Utc>>,
    pub effective_from: DateTime<Utc>,
    pub effective_to: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Rules & Policy Types
// ============================================================================

/// Boundary-owned policy container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ruleset {
    pub ruleset_id: Uuid,
    pub owner_type: String,
    pub owner_id: Option<Uuid>,
    pub name: String,
    pub ruleset_boundary: String,
    pub version: i32,
    pub effective_from: DateTime<Utc>,
    pub effective_to: Option<DateTime<Utc>>,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Individual rule within a ruleset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub rule_id: Uuid,
    pub ruleset_id: Uuid,
    pub name: String,
    pub kind: String,
    pub when_expr: serde_json::Value,
    pub then_effect: serde_json::Value,
    pub explain: Option<String>,
    pub priority: i32,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Closed-world field dictionary entry for rule validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFieldDictionaryEntry {
    pub field_key: String,
    pub field_type: String,
    pub description: Option<String>,
    pub source_table: Option<String>,
    pub added_in_version: i32,
}

// ============================================================================
// Contract Types
// ============================================================================

/// Template pack for contract generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractPack {
    pub contract_pack_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub effective_from: DateTime<Utc>,
    pub effective_to: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Individual template within a contract pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractTemplate {
    pub contract_template_id: Uuid,
    pub contract_pack_id: Uuid,
    pub template_type: String,
    pub template_ref: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Evaluation Types (Tier 3 — append-only audit records)
// ============================================================================

/// Immutable eligibility evaluation audit record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EligibilityEvaluation {
    pub eligibility_evaluation_id: Uuid,
    pub client_profile_id: Uuid,
    pub client_group_id: Uuid,
    pub product_offering_ids: Vec<Uuid>,
    pub requested_at: DateTime<Utc>,
    pub requested_by: String,
    pub policy_snapshot: serde_json::Value,
    pub evaluation_context: Option<serde_json::Value>,
    pub result: serde_json::Value,
    pub explain: serde_json::Value,
    pub selected_principal_id: Option<Uuid>,
    pub selected_at: Option<DateTime<Utc>>,
    pub runbook_entry_id: Option<Uuid>,
}

// ============================================================================
// Domain Enums — boundary-aware evaluation logic
// ============================================================================

/// Which barrier classification a ruleset/gate operates under
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RulesetBoundary {
    Regulatory,
    Commercial,
    Operational,
}

impl RulesetBoundary {
    pub fn from_str_val(s: &str) -> Option<Self> {
        match s {
            "regulatory" => Some(Self::Regulatory),
            "commercial" => Some(Self::Commercial),
            "operational" => Some(Self::Operational),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Regulatory => "regulatory",
            Self::Commercial => "commercial",
            Self::Operational => "operational",
        }
    }
}

/// Classified status of a candidate principal after evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CandidateStatus {
    Eligible,
    EligibleWithGates {
        gates: Vec<Gate>,
    },
    ConditionalDeny {
        boundary: RulesetBoundary,
        reason: String,
        override_gate: Option<Gate>,
        blocking_rules: Vec<Uuid>,
    },
    HardDeny {
        reason: String,
        regulation_ref: Option<String>,
        blocking_rules: Vec<Uuid>,
    },
}

/// Gate severity determines how it manifests in the runbook
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateSeverity {
    Blocking,
    Advisory,
}

/// A gate required before proceeding, tagged with boundary origin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gate {
    pub gate_code: String,
    pub gate_name: String,
    pub boundary: RulesetBoundary,
    pub severity: GateSeverity,
    pub source_rule_id: Uuid,
    pub source_ruleset_id: Uuid,
}

// ============================================================================
// Rule Expression Types — structured JSON conditions and effects
// ============================================================================

/// Structured condition tree for rule when_expr evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),
    Field {
        field: String,
        op: Operator,
        value: serde_json::Value,
    },
}

/// Comparison operators for field conditions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Neq,
    In,
    NotIn,
    Contains,
    Exists,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// Structured outcome of a rule evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Effect {
    Deny {
        reason_code: String,
        reason: String,
    },
    RequireGate {
        gate: String,
        severity: GateSeverity,
    },
    Allow,
    ConstrainPrincipal {
        field: String,
        op: Operator,
        value: serde_json::Value,
    },
    SelectContract {
        contract_pack_code: String,
        template_types: Vec<String>,
    },
}

// ============================================================================
// Evaluation Result Types — structured output from booking-principal.evaluate
// ============================================================================

/// A candidate principal with boundary-classified status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatedCandidate {
    pub principal_id: Uuid,
    pub principal_code: String,
    pub legal_entity_name: String,
    pub score: f64,
    pub status: CandidateStatus,
    pub existing_relationship: bool,
    pub existing_offerings: Vec<String>,
    pub reasons: Vec<String>,
}

/// Gate applicable to specific candidates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationGate {
    pub gate_code: String,
    pub gate_name: String,
    pub boundary: RulesetBoundary,
    pub severity: GateSeverity,
    pub source_rule_id: Uuid,
    pub applies_to_principal_ids: Vec<Uuid>,
}

/// Contract pack selected by rules for specific candidates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContractPack {
    pub contract_pack_code: String,
    pub contract_pack_name: String,
    pub template_types: Vec<String>,
    pub applies_to_principal_ids: Vec<Uuid>,
}

/// Per-principal service delivery feasibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryPlanEntry {
    pub principal_id: Uuid,
    pub service_code: String,
    pub regulatory_status: String,
    pub commercial_status: String,
    pub operational_status: String,
    pub delivery_model: Option<String>,
    pub available: bool,
    pub constraints_evaluated: Option<serde_json::Value>,
}

/// Rule explanation entry in the explain payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainEntry {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub ruleset_boundary: RulesetBoundary,
    pub kind: String,
    pub outcome: String,
    pub evaluated_facts: serde_json::Value,
    pub merge_decision: Option<String>,
}

/// Full evaluation result returned by booking-principal.evaluate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub evaluation_id: Uuid,
    pub candidates: Vec<EvaluatedCandidate>,
    pub gates: Vec<EvaluationGate>,
    pub contract_packs: Vec<EvaluationContractPack>,
    pub delivery_plan: Vec<DeliveryPlanEntry>,
    pub explain: Vec<ExplainEntry>,
    pub policy_snapshot: serde_json::Value,
}

/// Result of booking-principal.select
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResult {
    pub selected_principal_id: Uuid,
    pub contract_packs: Vec<String>,
    pub gates: Vec<Gate>,
    pub override_required: bool,
    pub override_gate: Option<Gate>,
}

/// Coverage matrix entry for a segment x jurisdiction x principal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageMatrixEntry {
    pub segment: String,
    pub jurisdiction: String,
    pub principal_id: Uuid,
    pub principal_code: String,
    pub regulatory: String,
    pub commercial: String,
    pub operational: String,
    pub overall: String,
}

/// Gap report entry with boundary classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapReportEntry {
    pub offering_code: Option<String>,
    pub jurisdiction: String,
    pub principal_code: Option<String>,
    pub gap_type: String,
    pub detail: String,
    pub delivery_model: Option<String>,
}

/// Impact analysis entry for principal retirement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisEntry {
    pub client_group_id: Uuid,
    pub offering_code: String,
    pub relationship_status: String,
    pub alternative_principals: Vec<EvaluatedCandidate>,
}

/// Deal context for market-scope evaluation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DealContext {
    pub market_countries: Option<Vec<String>>,
    pub instrument_types: Option<Vec<String>>,
    pub trading_venues: Option<Vec<String>>,
}
