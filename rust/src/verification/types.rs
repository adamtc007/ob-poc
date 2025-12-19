//! Core types for the Adversarial Verification Model
//!
//! These types wrap and extend existing observation/allegation infrastructure
//! to provide a game-theoretic verification perspective.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Claim Types - What the client asserts
// ============================================================================

/// A claim is an assertion made by the client that requires verification.
/// This wraps/extends `client_allegations` with adversarial context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// Unique identifier (maps to allegation_id)
    pub claim_id: Uuid,

    /// CBU context
    pub cbu_id: Uuid,

    /// Entity this claim is about
    pub entity_id: Option<Uuid>,

    /// What is being claimed (attribute or relationship)
    pub claim_type: ClaimType,

    /// The claimed value
    pub claimed_value: serde_json::Value,

    /// Human-readable display value
    pub display_value: String,

    /// Source of the claim
    pub source: ClaimSource,

    /// Current verification status
    pub status: ClaimStatus,

    /// When the claim was made
    pub claimed_at: DateTime<Utc>,

    /// Who made the claim
    pub claimed_by: Option<String>,

    /// Linked evidence that supports or contradicts
    pub evidence_ids: Vec<Uuid>,

    /// Calculated confidence score (0.0 - 1.0)
    pub confidence: Option<f64>,
}

/// Type of claim being made
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimType {
    /// Claim about an entity attribute (name, DOB, address, etc.)
    Attribute,
    /// Claim about ownership percentage
    Ownership,
    /// Claim about control relationship
    Control,
    /// Claim about UBO status
    Ubo,
    /// Claim about source of funds/wealth
    SourceOfFunds,
    /// Claim about business purpose
    BusinessPurpose,
}

/// Source of the claim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimSource {
    /// Onboarding form submission
    OnboardingForm,
    /// KYC questionnaire response
    KycQuestionnaire,
    /// Email correspondence
    Email,
    /// Verbal statement
    Verbal,
    /// API submission
    Api,
    /// Extracted from document
    Document,
    /// Carried forward from prior case
    PriorCase,
}

/// Verification status of a claim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimStatus {
    /// Claim submitted, not yet reviewed
    Pending,
    /// Verification in progress
    InProgress,
    /// Claim verified by evidence
    Verified,
    /// Claim contradicted by evidence
    Contradicted,
    /// Partially verified (acceptable variations)
    Partial,
    /// Cannot be verified (no authoritative source)
    Unverifiable,
    /// Verification waived with justification
    Waived,
    /// Claim is under formal challenge
    Challenged,
}

// ============================================================================
// Evidence Types - What supports or contradicts claims
// ============================================================================

/// Evidence is an observation from an authoritative source.
/// This wraps `attribute_observations` with adversarial context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Unique identifier (maps to observation_id)
    pub evidence_id: Uuid,

    /// Entity this evidence is about
    pub entity_id: Uuid,

    /// What attribute this evidences
    pub attribute_id: Uuid,

    /// The observed value
    pub observed_value: serde_json::Value,

    /// Source of the evidence
    pub source: EvidenceSource,

    /// Confidence score from source (0.0 - 1.0)
    pub confidence: f64,

    /// Is this from an authoritative source?
    pub is_authoritative: bool,

    /// When the evidence was collected
    pub observed_at: DateTime<Utc>,

    /// Document ID if extracted from document
    pub source_document_id: Option<Uuid>,

    /// Extraction method (OCR, MRZ, API, etc.)
    pub extraction_method: Option<String>,

    /// Temporal validity
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_to: Option<DateTime<Utc>>,
}

/// Source type for evidence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceSource {
    /// Government registry (company house, LEI, etc.)
    GovernmentRegistry,
    /// Regulated financial institution
    RegulatedEntity,
    /// Audited financial statements
    AuditedFinancial,
    /// Document extraction
    Document,
    /// Third-party data provider
    ThirdParty,
    /// Screening service result
    Screening,
    /// System-derived (calculated)
    System,
    /// Manual entry by analyst
    Manual,
    /// Client allegation (low trust)
    Allegation,
}

impl EvidenceSource {
    /// Get the base confidence weight for this source type
    pub fn base_confidence(&self) -> f64 {
        match self {
            EvidenceSource::GovernmentRegistry => 0.95,
            EvidenceSource::RegulatedEntity => 0.90,
            EvidenceSource::AuditedFinancial => 0.85,
            EvidenceSource::Document => 0.70,
            EvidenceSource::ThirdParty => 0.60,
            EvidenceSource::Screening => 0.75,
            EvidenceSource::System => 0.80,
            EvidenceSource::Manual => 0.50,
            EvidenceSource::Allegation => 0.30,
        }
    }

    /// Is this an authoritative source by default?
    pub fn is_authoritative_by_default(&self) -> bool {
        matches!(
            self,
            EvidenceSource::GovernmentRegistry
                | EvidenceSource::RegulatedEntity
                | EvidenceSource::AuditedFinancial
        )
    }
}

// ============================================================================
// Inconsistency Types - When evidence conflicts
// ============================================================================

/// An inconsistency is detected when evidence conflicts.
/// This wraps `observation_discrepancies` with adversarial context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inconsistency {
    /// Unique identifier
    pub inconsistency_id: Uuid,

    /// Entity with conflicting evidence
    pub entity_id: Uuid,

    /// Attribute with conflict
    pub attribute_id: Uuid,

    /// First observation
    pub observation_1_id: Uuid,
    pub value_1: serde_json::Value,

    /// Second observation
    pub observation_2_id: Uuid,
    pub value_2: serde_json::Value,

    /// Type of inconsistency
    pub inconsistency_type: InconsistencyType,

    /// Severity of the discrepancy
    pub severity: InconsistencySeverity,

    /// When detected
    pub detected_at: DateTime<Utc>,

    /// Resolution status
    pub resolution_status: InconsistencyResolution,
}

/// Type of inconsistency detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InconsistencyType {
    /// Values are completely different
    ValueMismatch,
    /// Dates don't match
    DateMismatch,
    /// Minor spelling/format variation
    SpellingVariation,
    /// Different format, same underlying value
    FormatDifference,
    /// One source has value, other is missing
    MissingVsPresent,
    /// Values directly contradict each other
    Contradictory,
}

/// Severity of inconsistency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InconsistencySeverity {
    /// Informational only
    Info,
    /// Minor issue, can be resolved
    Low,
    /// Moderate issue, needs review
    Medium,
    /// Significant issue, requires investigation
    High,
    /// Critical issue, blocks progress
    Critical,
}

/// Resolution status for inconsistency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InconsistencyResolution {
    /// Not yet addressed
    Open,
    /// Under investigation
    Investigating,
    /// Resolved with accepted observation
    Resolved,
    /// Escalated to higher authority
    Escalated,
    /// Accepted as non-material
    Accepted,
}

// ============================================================================
// Challenge Types - Formal verification challenges
// ============================================================================

/// A formal challenge raised when verification fails or patterns detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    /// Unique identifier
    pub challenge_id: Uuid,

    /// CBU context
    pub cbu_id: Uuid,

    /// Related KYC case
    pub case_id: Option<Uuid>,

    /// Entity being challenged
    pub entity_id: Option<Uuid>,

    /// Related allegation (if challenging a claim)
    pub allegation_id: Option<Uuid>,

    /// Related observation (if challenging evidence)
    pub observation_id: Option<Uuid>,

    /// Type of challenge
    pub challenge_type: ChallengeType,

    /// Why the challenge was raised
    pub challenge_reason: String,

    /// Severity of the challenge
    pub severity: ChallengeSeverity,

    /// Current status
    pub status: ChallengeStatus,

    /// Client's response
    pub response_text: Option<String>,

    /// Evidence provided in response
    pub response_evidence_ids: Vec<Uuid>,

    /// When raised
    pub raised_at: DateTime<Utc>,

    /// Who raised it
    pub raised_by: Option<String>,

    /// When responded
    pub responded_at: Option<DateTime<Utc>>,

    /// When resolved
    pub resolved_at: Option<DateTime<Utc>>,

    /// Who resolved
    pub resolved_by: Option<String>,

    /// Resolution type
    pub resolution_type: Option<ChallengeResolution>,

    /// Resolution notes
    pub resolution_notes: Option<String>,
}

/// Type of challenge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChallengeType {
    /// Inconsistency between observations
    Inconsistency,
    /// Confidence score too low
    LowConfidence,
    /// Missing corroborating evidence
    MissingCorroboration,
    /// Suspicious pattern detected
    PatternDetected,
    /// Registry verification failed
    RegistryMismatch,
    /// Evasion behavior detected
    EvasionDetected,
    /// Missing required information
    MissingRequired,
}

/// Severity of challenge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChallengeSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Challenge workflow status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChallengeStatus {
    /// Challenge raised, awaiting response
    Open,
    /// Client has responded
    Responded,
    /// Challenge resolved
    Resolved,
    /// Escalated to higher authority
    Escalated,
}

/// How the challenge was resolved
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChallengeResolution {
    /// Response accepted, challenge cleared
    Accepted,
    /// Response rejected, issue stands
    Rejected,
    /// Challenge waived with justification
    Waived,
    /// Escalated to higher authority
    Escalated,
}

// ============================================================================
// Escalation Types - Risk-based routing
// ============================================================================

/// An escalation to higher authority for decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Escalation {
    /// Unique identifier
    pub escalation_id: Uuid,

    /// CBU context
    pub cbu_id: Uuid,

    /// Related KYC case
    pub case_id: Option<Uuid>,

    /// Related challenge (if escalating a challenge)
    pub challenge_id: Option<Uuid>,

    /// Level to escalate to
    pub escalation_level: EscalationLevel,

    /// Why escalating
    pub escalation_reason: String,

    /// Risk indicators that triggered escalation
    pub risk_indicators: serde_json::Value,

    /// Current status
    pub status: EscalationStatus,

    /// Decision made
    pub decision: Option<EscalationDecision>,

    /// Decision notes
    pub decision_notes: Option<String>,

    /// When escalated
    pub escalated_at: DateTime<Utc>,

    /// Who escalated
    pub escalated_by: Option<String>,

    /// When decided
    pub decided_at: Option<DateTime<Utc>>,

    /// Who decided
    pub decided_by: Option<String>,
}

/// Escalation authority level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EscalationLevel {
    /// Senior analyst review
    SeniorAnalyst,
    /// Compliance officer
    ComplianceOfficer,
    /// Money Laundering Reporting Officer
    Mlro,
    /// Risk committee
    Committee,
    /// Board level
    Board,
}

/// Escalation workflow status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EscalationStatus {
    /// Awaiting review
    Pending,
    /// Under review
    UnderReview,
    /// Decision made
    Decided,
}

/// Escalation decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EscalationDecision {
    /// Approve despite issues
    Approve,
    /// Reject due to issues
    Reject,
    /// Require more information
    RequireMoreInfo,
    /// Escalate to next level
    EscalateFurther,
}

// ============================================================================
// Utility Implementations
// ============================================================================

impl From<&str> for EvidenceSource {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GOVERNMENT_REGISTRY" => EvidenceSource::GovernmentRegistry,
            "REGULATED_ENTITY" => EvidenceSource::RegulatedEntity,
            "AUDITED_FINANCIAL" => EvidenceSource::AuditedFinancial,
            "DOCUMENT" => EvidenceSource::Document,
            "THIRD_PARTY" => EvidenceSource::ThirdParty,
            "SCREENING" => EvidenceSource::Screening,
            "SYSTEM" | "DERIVED" => EvidenceSource::System,
            "MANUAL" => EvidenceSource::Manual,
            "ALLEGATION" | "CLIENT_ALLEGATION" => EvidenceSource::Allegation,
            _ => EvidenceSource::Manual, // Default fallback
        }
    }
}

impl std::fmt::Display for ClaimStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaimStatus::Pending => write!(f, "PENDING"),
            ClaimStatus::InProgress => write!(f, "IN_PROGRESS"),
            ClaimStatus::Verified => write!(f, "VERIFIED"),
            ClaimStatus::Contradicted => write!(f, "CONTRADICTED"),
            ClaimStatus::Partial => write!(f, "PARTIAL"),
            ClaimStatus::Unverifiable => write!(f, "UNVERIFIABLE"),
            ClaimStatus::Waived => write!(f, "WAIVED"),
            ClaimStatus::Challenged => write!(f, "CHALLENGED"),
        }
    }
}

impl std::fmt::Display for ChallengeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChallengeStatus::Open => write!(f, "OPEN"),
            ChallengeStatus::Responded => write!(f, "RESPONDED"),
            ChallengeStatus::Resolved => write!(f, "RESOLVED"),
            ChallengeStatus::Escalated => write!(f, "ESCALATED"),
        }
    }
}

impl std::fmt::Display for EscalationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationStatus::Pending => write!(f, "PENDING"),
            EscalationStatus::UnderReview => write!(f, "UNDER_REVIEW"),
            EscalationStatus::Decided => write!(f, "DECIDED"),
        }
    }
}
