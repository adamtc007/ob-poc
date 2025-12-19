# TODO: Adversarial Agent Model (Game Theory / Trust But Verify)

## ⛔ MANDATORY FIRST STEP

**Read these files before starting:**
- `/EGUI-RULES.md` - UI patterns and constraints
- `/rust/config/verbs/` - All existing verb definitions
- `/rust/src/dsl_v2/runtime_registry.rs` - Verb execution model
- `/TODO-GRAPH-DSL-DOMAIN.md` - Graph query verbs (dependency)
- `/ALLIANZ-DATA-ACQUISITION.md` - Context on UBO data model

---

## Overview: The Game

This is NOT a workflow system. This is an **adversarial verification game**.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE GAME                                                                   │
│                                                                             │
│  PLAYER 1: AGENT (BNY)                 PLAYER 2: CLIENT                    │
│  ═════════════════════                 ════════════════                     │
│                                                                             │
│  GOAL: Establish TRUE UBO              GOAL: Achieve KYC clearance         │
│                                                                             │
│  WIN: Verified truth, would            LEGITIMATE: Wins with truth         │
│       catch a sophisticated liar       ADVERSARIAL: Wins with lies         │
│                                                                             │
│  LOSE: Accept unverified claims        ADVERSARIAL WINS: False clearance   │
│        (even if client honest)                                             │
│                                                                             │
│  ───────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  THE BOARD: CBU/UBO Graph                                                  │
│                                                                             │
│  Every node and edge has:                                                  │
│  - CLAIMED state (what client says)                                        │
│  - VERIFIED state (what we've proven)                                      │
│  - CONFIDENCE score (how sure are we?)                                     │
│  - EVIDENCE chain (what supports this?)                                    │
│                                                                             │
│  ───────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  WINNING CONDITION:                                                         │
│                                                                             │
│  NOT: "All gaps closed" (checklist thinking)                               │
│  BUT: "All claims verified with high confidence" (adversarial thinking)    │
│                                                                             │
│  Standard: WOULD THIS CATCH A SOPHISTICATED LIAR?                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Core Principle: Trust But Verify → Distrust And Verify

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  OLD THINKING                          NEW THINKING                         │
│  ════════════                          ════════════                         │
│                                                                             │
│  Gap = "missing data"                  Gap = "unverified claim"            │
│  Close gap = "get data"                Close gap = "verify claim"          │
│  Done = "all data present"             Done = "all claims proven"          │
│                                                                             │
│  Agent fills forms                     Agent plays adversary               │
│  Client provides info                  Client proves claims                │
│  Trust by default                      Distrust by default                 │
│                                                                             │
│  Success = checklist complete          Success = truth established         │
│  Failure = checklist incomplete        Failure = false claim accepted      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Claim & Verification Model

### 1.1 Core Types

**File:** `rust/src/verification/types.rs`

```rust
//! Verification Model Types
//!
//! Every piece of information is a CLAIM that must be VERIFIED.

use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A claim about something - could be true or false
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// Unique claim ID
    pub id: Uuid,
    
    /// What is being claimed
    pub claim_type: ClaimType,
    
    /// Subject of the claim (entity, relationship, attribute)
    pub subject_id: Uuid,
    pub subject_type: String,
    
    /// The actual claim content
    pub content: ClaimContent,
    
    /// Who made this claim
    pub source: ClaimSource,
    
    /// When was it claimed
    pub claimed_at: DateTime<Utc>,
    
    /// Current verification status
    pub verification: VerificationStatus,
    
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    
    /// Evidence supporting or refuting
    pub evidence: Vec<Evidence>,
    
    /// Any inconsistencies detected
    pub inconsistencies: Vec<Inconsistency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClaimType {
    /// Entity exists with these attributes
    EntityExists,
    /// A owns B with X%
    Ownership { percentage: f32 },
    /// A controls B (director, officer)
    Control { role: String },
    /// Person identity
    PersonIdentity,
    /// Jurisdiction of entity
    Jurisdiction,
    /// Entity is regulated/listed (exemption claim)
    RegulatoryStatus,
    /// Document is authentic
    DocumentAuthenticity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimContent {
    /// Structured claim data
    pub data: serde_json::Value,
    /// Human readable description
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimSource {
    /// Source type
    pub source_type: SourceType,
    /// Specific source identifier
    pub source_id: Option<String>,
    /// Document reference if applicable
    pub document_id: Option<Uuid>,
    /// When was this source consulted
    pub retrieved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SourceType {
    /// Direct government registry API
    GovernmentRegistry,
    /// GLEIF LEI database
    Gleif,
    /// Stock exchange listing
    ExchangeListing,
    /// Regulatory filing (SEC, CSSF, etc.)
    RegulatoryFiling,
    /// Audited financial statements
    AuditedFinancials,
    /// Notarized document
    NotarizedDocument,
    /// Client-provided certified document
    ClientCertified,
    /// Client-provided uncertified document
    ClientUncertified,
    /// Verbal or email claim
    VerbalClaim,
    /// Screening provider
    ScreeningProvider,
    /// Internal system
    InternalSystem,
}

impl SourceType {
    /// Base confidence for this source type
    pub fn base_confidence(&self) -> f32 {
        match self {
            Self::GovernmentRegistry => 0.95,
            Self::Gleif => 0.90,
            Self::ExchangeListing => 0.90,
            Self::RegulatoryFiling => 0.85,
            Self::AuditedFinancials => 0.80,
            Self::NotarizedDocument => 0.75,
            Self::ClientCertified => 0.60,
            Self::ClientUncertified => 0.40,
            Self::VerbalClaim => 0.20,
            Self::ScreeningProvider => 0.85,
            Self::InternalSystem => 0.70,
        }
    }
    
    /// Is this an independent source (not from client)?
    pub fn is_independent(&self) -> bool {
        match self {
            Self::GovernmentRegistry => true,
            Self::Gleif => true,
            Self::ExchangeListing => true,
            Self::RegulatoryFiling => true,
            Self::AuditedFinancials => true,
            Self::ScreeningProvider => true,
            Self::NotarizedDocument => false, // Client obtained it
            Self::ClientCertified => false,
            Self::ClientUncertified => false,
            Self::VerbalClaim => false,
            Self::InternalSystem => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationStatus {
    /// Overall status
    pub status: VerificationState,
    /// When last verified
    pub verified_at: Option<DateTime<Utc>>,
    /// Who/what verified it
    pub verified_by: Option<String>,
    /// Verification method used
    pub method: Option<VerificationMethod>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationState {
    /// Claim made but not yet verified
    Claimed,
    /// Verification in progress
    Verifying,
    /// Verified by independent source(s)
    Verified,
    /// Multiple sources corroborate
    Corroborated,
    /// Could not verify (no source found)
    Unverifiable,
    /// Verification found inconsistencies
    Disputed,
    /// Verified as FALSE
    Refuted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VerificationMethod {
    /// Direct registry lookup
    RegistryLookup,
    /// GLEIF API check
    GleifVerification,
    /// Document examination
    DocumentReview,
    /// Cross-reference multiple sources
    CrossReference,
    /// Screening check
    ScreeningCheck,
    /// Human analyst review
    HumanReview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Evidence ID
    pub id: Uuid,
    /// Type of evidence
    pub evidence_type: EvidenceType,
    /// Source of this evidence
    pub source: ClaimSource,
    /// Does this support or refute the claim?
    pub supports_claim: bool,
    /// Confidence contribution
    pub confidence_impact: f32,
    /// Reference to document/data
    pub reference: Option<String>,
    /// When obtained
    pub obtained_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EvidenceType {
    /// Official document
    Document,
    /// Registry record
    RegistryRecord,
    /// API response data
    ApiData,
    /// Screening result
    ScreeningResult,
    /// Corroborating claim from another source
    Corroboration,
    /// Human analyst note
    AnalystNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inconsistency {
    /// What is inconsistent
    pub description: String,
    /// Conflicting sources
    pub source_a: ClaimSource,
    pub source_b: ClaimSource,
    /// Value from source A
    pub value_a: String,
    /// Value from source B
    pub value_b: String,
    /// Severity
    pub severity: InconsistencySeverity,
    /// Is this resolved?
    pub resolved: bool,
    /// Resolution notes
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InconsistencySeverity {
    /// Minor discrepancy (e.g., spelling)
    Minor,
    /// Significant but explainable
    Moderate,
    /// Serious - needs investigation
    Serious,
    /// Critical - potential fraud indicator
    Critical,
}
```

### 1.2 Confidence Calculator

**File:** `rust/src/verification/confidence.rs`

```rust
//! Confidence Score Calculator
//!
//! Calculates confidence scores for claims based on evidence and modifiers.

use super::types::*;

/// Calculate confidence score for a claim
pub fn calculate_confidence(claim: &Claim) -> f32 {
    // Start with source base confidence
    let mut confidence = claim.source.source_type.base_confidence();
    
    // Apply evidence modifiers
    for evidence in &claim.evidence {
        if evidence.supports_claim {
            confidence += evidence.confidence_impact;
        } else {
            confidence -= evidence.confidence_impact;
        }
    }
    
    // Apply corroboration bonus
    let corroboration_count = count_independent_corroborations(claim);
    if corroboration_count >= 2 {
        confidence += 0.15;
    } else if corroboration_count == 1 {
        confidence += 0.08;
    }
    
    // Apply inconsistency penalties
    for inconsistency in &claim.inconsistencies {
        if !inconsistency.resolved {
            let penalty = match inconsistency.severity {
                InconsistencySeverity::Minor => 0.05,
                InconsistencySeverity::Moderate => 0.15,
                InconsistencySeverity::Serious => 0.30,
                InconsistencySeverity::Critical => 0.50,
            };
            confidence -= penalty;
        }
    }
    
    // Apply pattern detection penalties
    // (These would come from pattern detection engine)
    
    // Clamp to valid range
    confidence.clamp(0.0, 1.0)
}

fn count_independent_corroborations(claim: &Claim) -> usize {
    claim.evidence.iter()
        .filter(|e| e.supports_claim && e.source.source_type.is_independent())
        .count()
}

/// Confidence thresholds
pub mod thresholds {
    /// Verified - high confidence, can proceed
    pub const VERIFIED: f32 = 0.80;
    /// Provisional - needs additional evidence
    pub const PROVISIONAL: f32 = 0.60;
    /// Unverified - actively challenge
    pub const UNVERIFIED: f32 = 0.40;
    /// Below this - escalate immediately
    pub const SUSPECT: f32 = 0.40;
}

/// Get verification state from confidence score
pub fn confidence_to_state(confidence: f32, has_inconsistencies: bool) -> VerificationState {
    if has_inconsistencies {
        return VerificationState::Disputed;
    }
    
    if confidence >= thresholds::VERIFIED {
        VerificationState::Verified
    } else if confidence >= thresholds::PROVISIONAL {
        VerificationState::Claimed // Needs more evidence
    } else if confidence >= thresholds::SUSPECT {
        VerificationState::Unverifiable
    } else {
        VerificationState::Disputed // Too low, treat as suspect
    }
}

/// Confidence modifiers
pub mod modifiers {
    /// Multiple sources agree
    pub const CORROBORATION_BONUS: f32 = 0.15;
    /// Document less than 3 months old
    pub const RECENT_DOCUMENT_BONUS: f32 = 0.05;
    /// Known entity from existing relationship
    pub const KNOWN_ENTITY_BONUS: f32 = 0.10;
    
    /// Inconsistency with other source
    pub const INCONSISTENCY_PENALTY: f32 = 0.30;
    /// High-risk jurisdiction
    pub const HIGH_RISK_JURISDICTION_PENALTY: f32 = 0.15;
    /// Document older than 12 months
    pub const STALE_DOCUMENT_PENALTY: f32 = 0.10;
    /// Nominee pattern detected
    pub const NOMINEE_PATTERN_PENALTY: f32 = 0.25;
    /// Evasion pattern detected
    pub const EVASION_PATTERN_PENALTY: f32 = 0.40;
}
```

### 1.3 Tasks - Claim & Verification Model

- [ ] Create `rust/src/verification/` module
- [ ] Create `rust/src/verification/types.rs`
- [ ] Implement `Claim` struct with all fields
- [ ] Implement `SourceType` with confidence values
- [ ] Implement `VerificationState` enum
- [ ] Create `rust/src/verification/confidence.rs`
- [ ] Implement confidence calculator
- [ ] Implement threshold checks
- [ ] Unit tests for confidence calculation

---

## Part 2: Adversarial Detection

### 2.1 Pattern Detection

**File:** `rust/src/verification/patterns.rs`

```rust
//! Adversarial Pattern Detection
//!
//! Detects patterns that may indicate deception, evasion, or fraud.

use uuid::Uuid;
use super::types::*;

/// Detected pattern that may indicate adversarial behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    /// Pattern ID
    pub id: Uuid,
    /// Pattern type
    pub pattern_type: PatternType,
    /// Entities involved
    pub entities: Vec<Uuid>,
    /// Description
    pub description: String,
    /// Risk level
    pub risk_level: RiskLevel,
    /// Confidence that this is actually the pattern
    pub detection_confidence: f32,
    /// Recommended action
    pub recommended_action: RecommendedAction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatternType {
    // Structural patterns
    /// A owns B owns C owns A
    CircularOwnership,
    /// Long chain of single-entity ownership
    LayeringStructure,
    /// Multiple shell companies in opacity jurisdictions
    ShellCompanyCluster,
    /// Nominee directors/shareholders detected
    NomineeUsage,
    
    // Behavioral patterns
    /// Repeated delays on specific document requests
    DocumentEvasion,
    /// Providing incomplete information repeatedly
    PartialDisclosure,
    /// Changing story when challenged
    InconsistentNarrative,
    /// Unexplained complexity
    UnnecessaryComplexity,
    
    // Data patterns
    /// GLEIF says X, client says Y
    RegistryMismatch,
    /// Document dates don't align with claimed timeline
    TimelineMismatch,
    /// Same address/directors across unrelated entities
    SuspiciousOverlap,
    /// Documents appear altered
    DocumentTampering,
    
    // Jurisdiction patterns
    /// Heavy use of secrecy jurisdictions
    OpacityJurisdictionUsage,
    /// Structure splits across high-risk jurisdictions
    JurisdictionShopping,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendedAction {
    /// Note but continue
    Monitor,
    /// Request additional evidence
    RequestEvidence { evidence_types: Vec<String> },
    /// Challenge with specific questions
    Challenge { questions: Vec<String> },
    /// Escalate to human review
    Escalate { reason: String },
    /// Reject / cannot proceed
    Reject { reason: String },
}

/// Pattern detection engine
pub struct PatternDetector {
    /// Known opacity jurisdictions
    opacity_jurisdictions: Vec<String>,
    /// Known nominee service providers
    known_nominee_providers: Vec<String>,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self {
            opacity_jurisdictions: vec![
                "VG".into(),  // British Virgin Islands
                "KY".into(),  // Cayman Islands
                "PA".into(),  // Panama
                "SC".into(),  // Seychelles
                "BZ".into(),  // Belize
                "WS".into(),  // Samoa
                "VU".into(),  // Vanuatu
            ],
            known_nominee_providers: vec![
                // Would be populated from reference data
            ],
        }
    }

    /// Analyze CBU for adversarial patterns
    pub async fn analyze_cbu(&self, cbu_id: Uuid, context: &Context) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        
        // Get all entities and relationships
        let graph = self.load_cbu_graph(cbu_id, context).await;
        
        // Check for circular ownership
        if let Some(cycle) = self.detect_circular_ownership(&graph) {
            patterns.push(DetectedPattern {
                id: Uuid::new_v4(),
                pattern_type: PatternType::CircularOwnership,
                entities: cycle,
                description: "Circular ownership detected - A owns B owns C owns A".into(),
                risk_level: RiskLevel::Critical,
                detection_confidence: 1.0,
                recommended_action: RecommendedAction::Escalate {
                    reason: "Circular ownership is a strong indicator of structure manipulation".into()
                },
            });
        }
        
        // Check for layering
        if let Some(chain) = self.detect_layering(&graph, 5) {
            patterns.push(DetectedPattern {
                id: Uuid::new_v4(),
                pattern_type: PatternType::LayeringStructure,
                entities: chain,
                description: format!("Deep ownership chain ({} layers) with single-entity ownership", chain.len()),
                risk_level: RiskLevel::High,
                detection_confidence: 0.8,
                recommended_action: RecommendedAction::Challenge {
                    questions: vec![
                        "What is the business purpose for this structure?".into(),
                        "Why are intermediate entities necessary?".into(),
                    ]
                },
            });
        }
        
        // Check for opacity jurisdiction usage
        let opacity_count = self.count_opacity_jurisdictions(&graph);
        if opacity_count >= 2 {
            patterns.push(DetectedPattern {
                id: Uuid::new_v4(),
                pattern_type: PatternType::OpacityJurisdictionUsage,
                entities: self.entities_in_opacity_jurisdictions(&graph),
                description: format!("{} entities in high-secrecy jurisdictions", opacity_count),
                risk_level: if opacity_count >= 3 { RiskLevel::High } else { RiskLevel::Medium },
                detection_confidence: 1.0,
                recommended_action: RecommendedAction::RequestEvidence {
                    evidence_types: vec![
                        "Business rationale for jurisdiction selection".into(),
                        "Source of funds documentation".into(),
                    ]
                },
            });
        }
        
        // Check for nominee patterns
        patterns.extend(self.detect_nominee_patterns(&graph));
        
        // Check for registry mismatches
        patterns.extend(self.detect_registry_mismatches(&graph).await);
        
        patterns
    }

    /// Detect circular ownership (A → B → C → A)
    fn detect_circular_ownership(&self, graph: &CbuGraph) -> Option<Vec<Uuid>> {
        // DFS with cycle detection
        let mut visited = std::collections::HashSet::new();
        let mut path = Vec::new();
        
        for entity_id in graph.entities.keys() {
            if let Some(cycle) = self.dfs_cycle(graph, *entity_id, &mut visited, &mut path) {
                return Some(cycle);
            }
        }
        None
    }

    /// Detect layering (long single-owner chains)
    fn detect_layering(&self, graph: &CbuGraph, min_depth: usize) -> Option<Vec<Uuid>> {
        // Find chains where each entity has exactly one parent
        for entity_id in graph.entities.keys() {
            let chain = self.trace_single_owner_chain(graph, *entity_id);
            if chain.len() >= min_depth {
                return Some(chain);
            }
        }
        None
    }

    /// Detect nominee usage patterns
    fn detect_nominee_patterns(&self, graph: &CbuGraph) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        
        for (entity_id, entity) in &graph.entities {
            // Check for known nominee indicators
            let nominee_indicators = vec![
                entity.name.to_lowercase().contains("nominee"),
                entity.name.to_lowercase().contains("trustee services"),
                entity.name.to_lowercase().contains("corporate services"),
                // Same director across multiple unrelated entities
                self.shares_directors_with_unrelated(graph, *entity_id),
                // Registered agent address (not real business address)
                self.is_registered_agent_address(&entity.address),
            ];
            
            let indicator_count = nominee_indicators.iter().filter(|&&x| x).count();
            
            if indicator_count >= 2 {
                patterns.push(DetectedPattern {
                    id: Uuid::new_v4(),
                    pattern_type: PatternType::NomineeUsage,
                    entities: vec![*entity_id],
                    description: format!("{} nominee indicators detected", indicator_count),
                    risk_level: RiskLevel::High,
                    detection_confidence: 0.7 + (indicator_count as f32 * 0.1),
                    recommended_action: RecommendedAction::Challenge {
                        questions: vec![
                            "Confirm whether nominee arrangements are in place".into(),
                            "If nominees used, provide details of nominator/beneficial owner".into(),
                            "Provide documentation of nominee arrangement".into(),
                        ]
                    },
                });
            }
        }
        
        patterns
    }

    /// Detect mismatches between claimed data and registry data
    async fn detect_registry_mismatches(&self, graph: &CbuGraph) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();
        
        for (entity_id, entity) in &graph.entities {
            // For each entity with an LEI, verify against GLEIF
            if let Some(lei) = &entity.lei {
                if let Some(gleif_data) = self.fetch_gleif(lei).await {
                    // Check for mismatches
                    if let Some(mismatch) = self.compare_to_gleif(entity, &gleif_data) {
                        patterns.push(DetectedPattern {
                            id: Uuid::new_v4(),
                            pattern_type: PatternType::RegistryMismatch,
                            entities: vec![*entity_id],
                            description: mismatch.description,
                            risk_level: mismatch.severity.into(),
                            detection_confidence: 1.0, // Registry data is authoritative
                            recommended_action: RecommendedAction::Challenge {
                                questions: vec![
                                    format!("GLEIF shows {}, but you claimed {}. Please explain.", 
                                            mismatch.registry_value, mismatch.claimed_value),
                                ]
                            },
                        });
                    }
                }
            }
        }
        
        patterns
    }
}
```

### 2.2 Evasion Detection

**File:** `rust/src/verification/evasion.rs`

```rust
//! Evasion Behavior Detection
//!
//! Detects behavioral patterns that suggest the client is evading verification.

/// Track document request history for evasion detection
#[derive(Debug, Clone)]
pub struct DocumentRequestHistory {
    pub entity_id: Uuid,
    pub document_type: String,
    pub requests: Vec<DocumentRequest>,
}

#[derive(Debug, Clone)]
pub struct DocumentRequest {
    pub request_id: Uuid,
    pub requested_at: DateTime<Utc>,
    pub status: RequestStatus,
    pub response: Option<RequestResponse>,
}

#[derive(Debug, Clone)]
pub enum RequestStatus {
    Pending,
    Received,
    Rejected,
    Expired,
}

#[derive(Debug, Clone)]
pub struct RequestResponse {
    pub received_at: DateTime<Utc>,
    pub document_id: Option<Uuid>,
    pub rejection_reason: Option<String>,
    pub delay_days: i32,
}

/// Evasion detector
pub struct EvasionDetector;

impl EvasionDetector {
    /// Analyze request history for evasion patterns
    pub fn analyze_request_history(
        &self,
        history: &[DocumentRequestHistory],
    ) -> Vec<EvasionIndicator> {
        let mut indicators = Vec::new();
        
        for doc_history in history {
            // Check for repeated delays
            let avg_delay = self.calculate_average_delay(&doc_history.requests);
            if avg_delay > 14.0 {
                indicators.push(EvasionIndicator {
                    indicator_type: EvasionType::RepeatedDelays,
                    entity_id: doc_history.entity_id,
                    description: format!(
                        "Average delay of {:.0} days for {} documents",
                        avg_delay, doc_history.document_type
                    ),
                    severity: if avg_delay > 30.0 { RiskLevel::High } else { RiskLevel::Medium },
                });
            }
            
            // Check for repeated rejections / wrong documents
            let rejection_rate = self.calculate_rejection_rate(&doc_history.requests);
            if rejection_rate > 0.3 {
                indicators.push(EvasionIndicator {
                    indicator_type: EvasionType::RepeatedRejections,
                    entity_id: doc_history.entity_id,
                    description: format!(
                        "{:.0}% of {} submissions rejected or wrong",
                        rejection_rate * 100.0, doc_history.document_type
                    ),
                    severity: RiskLevel::Medium,
                });
            }
            
            // Check for expired documents being provided
            // (Providing knowingly expired docs is potential evasion)
        }
        
        // Check for selective non-response
        // (Client responds quickly to some requests, delays others)
        if let Some(selective) = self.detect_selective_response(history) {
            indicators.push(selective);
        }
        
        indicators
    }
    
    /// Detect when client is quick on some docs but slow on others
    fn detect_selective_response(
        &self,
        history: &[DocumentRequestHistory],
    ) -> Option<EvasionIndicator> {
        let delays: Vec<(String, f32)> = history.iter()
            .map(|h| (h.document_type.clone(), self.calculate_average_delay(&h.requests)))
            .collect();
        
        if delays.len() < 2 {
            return None;
        }
        
        let min_delay = delays.iter().map(|(_, d)| *d).fold(f32::INFINITY, f32::min);
        let max_delay = delays.iter().map(|(_, d)| *d).fold(0.0f32, f32::max);
        
        // If variance is high, might be selective
        if max_delay > min_delay * 3.0 && max_delay > 14.0 {
            let slow_docs: Vec<&str> = delays.iter()
                .filter(|(_, d)| *d > min_delay * 2.0)
                .map(|(t, _)| t.as_str())
                .collect();
            
            Some(EvasionIndicator {
                indicator_type: EvasionType::SelectiveResponse,
                entity_id: Uuid::nil(), // Applies to whole case
                description: format!(
                    "Client responds quickly to some requests but delays: {}",
                    slow_docs.join(", ")
                ),
                severity: RiskLevel::Medium,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvasionIndicator {
    pub indicator_type: EvasionType,
    pub entity_id: Uuid,
    pub description: String,
    pub severity: RiskLevel,
}

#[derive(Debug, Clone, Copy)]
pub enum EvasionType {
    RepeatedDelays,
    RepeatedRejections,
    SelectiveResponse,
    ChangingExplanations,
    ExpiredDocuments,
    PartialDisclosure,
}
```

### 2.3 Tasks - Adversarial Detection

- [ ] Create `rust/src/verification/patterns.rs`
- [ ] Implement `PatternDetector`
- [ ] Implement circular ownership detection
- [ ] Implement layering detection
- [ ] Implement nominee pattern detection
- [ ] Implement registry mismatch detection
- [ ] Create `rust/src/verification/evasion.rs`
- [ ] Implement `EvasionDetector`
- [ ] Implement delay pattern detection
- [ ] Implement selective response detection
- [ ] Unit tests for pattern detection
- [ ] Integration tests with real graph data

---

## Part 3: Verification DSL Verbs

### 3.1 Verification Domain

**File:** `rust/config/verbs/verify.yaml`

```yaml
domains:
  verify:
    description: Verification and corroboration verbs (adversarial model)
    verbs:
      # =======================================================================
      # CLAIM MANAGEMENT
      # =======================================================================
      register-claim:
        description: Register a new claim (something client asserts)
        behavior: crud
        crud:
          operation: insert
          table: claims
          schema: ob-poc
          returning: claim_id
        args:
          - name: entity
            type: uuid
            required: true
            maps_to: subject_id
          - name: claim-type
            type: string
            required: true
            maps_to: claim_type
            valid_values:
              - entity_exists
              - ownership
              - control
              - person_identity
              - jurisdiction
              - regulatory_status
              - document_authenticity
          - name: content
            type: json
            required: true
            maps_to: claim_content
          - name: source-type
            type: string
            required: true
            maps_to: source_type
            valid_values:
              - government_registry
              - gleif
              - exchange_listing
              - regulatory_filing
              - audited_financials
              - notarized_document
              - client_certified
              - client_uncertified
              - verbal_claim
          - name: source-document
            type: uuid
            required: false
            maps_to: source_document_id
        defaults:
          verification_state: CLAIMED
        returns:
          type: uuid
          name: claim_id

      # =======================================================================
      # VERIFICATION ACTIONS
      # =======================================================================
      verify-against-gleif:
        description: Verify entity/ownership claim against GLEIF registry
        behavior: plugin
        plugin: verify_gleif
        args:
          - name: entity
            type: uuid
            required: true
          - name: claim
            type: uuid
            required: false
            description: Specific claim to verify, or verify all claims for entity
        returns:
          type: verification_result

      verify-against-registry:
        description: Verify claim against government/corporate registry
        behavior: plugin
        plugin: verify_registry
        args:
          - name: entity
            type: uuid
            required: true
          - name: registry
            type: string
            required: true
            valid_values:
              - companies_house_uk
              - handelsregister_de
              - rcs_luxembourg
              - cro_ireland
              - sec_edgar
          - name: claim
            type: uuid
            required: false
        returns:
          type: verification_result

      verify-document:
        description: Verify document authenticity and content
        behavior: plugin
        plugin: verify_document
        args:
          - name: document
            type: uuid
            required: true
          - name: checks
            type: string_array
            required: false
            default:
              - format_valid
              - not_expired
              - issuer_verified
              - content_matches_claim
        returns:
          type: verification_result

      # =======================================================================
      # CORROBORATION
      # =======================================================================
      corroborate:
        description: Cross-reference claim across multiple sources
        behavior: plugin
        plugin: corroborate_claim
        args:
          - name: claim
            type: uuid
            required: true
          - name: sources
            type: string_array
            required: true
            description: Sources to check for corroboration
        returns:
          type: corroboration_result

      check-consistency:
        description: Check claim for consistency with other known facts
        behavior: plugin
        plugin: check_consistency
        args:
          - name: entity
            type: uuid
            required: true
          - name: scope
            type: string
            required: false
            default: all_claims
            valid_values:
              - all_claims
              - ownership_only
              - identity_only
        returns:
          type: consistency_report

      # =======================================================================
      # ADVERSARIAL DETECTION
      # =======================================================================
      detect-patterns:
        description: Run adversarial pattern detection on CBU
        behavior: plugin
        plugin: detect_patterns
        args:
          - name: cbu
            type: uuid
            required: true
          - name: patterns
            type: string_array
            required: false
            description: Specific patterns to check, or all if not specified
            valid_values:
              - circular_ownership
              - layering
              - nominee_usage
              - opacity_jurisdictions
              - registry_mismatch
              - all
        returns:
          type: pattern_detection_result

      detect-evasion:
        description: Analyze client behavior for evasion patterns
        behavior: plugin
        plugin: detect_evasion
        args:
          - name: cbu
            type: uuid
            required: true
        returns:
          type: evasion_detection_result

      # =======================================================================
      # CHALLENGE & ESCALATE
      # =======================================================================
      challenge:
        description: Create a challenge requiring client response
        behavior: crud
        crud:
          operation: insert
          table: challenges
          schema: ob-poc
          returning: challenge_id
        args:
          - name: entity
            type: uuid
            required: true
            maps_to: entity_id
          - name: claim
            type: uuid
            required: false
            maps_to: claim_id
          - name: challenge-type
            type: string
            required: true
            maps_to: challenge_type
            valid_values:
              - inconsistency
              - missing_evidence
              - suspicious_pattern
              - expired_document
              - registry_mismatch
          - name: questions
            type: string_array
            required: true
            maps_to: questions
          - name: evidence-required
            type: string_array
            required: false
            maps_to: evidence_required
        returns:
          type: uuid
          name: challenge_id

      escalate:
        description: Escalate to human review with reason
        behavior: crud
        crud:
          operation: insert
          table: escalations
          schema: ob-poc
          returning: escalation_id
        args:
          - name: cbu
            type: uuid
            required: true
            maps_to: cbu_id
          - name: reason
            type: string
            required: true
            maps_to: reason
          - name: risk-level
            type: string
            required: true
            maps_to: risk_level
            valid_values:
              - medium
              - high
              - critical
          - name: patterns-detected
            type: uuid_array
            required: false
            maps_to: pattern_ids
          - name: claims-disputed
            type: uuid_array
            required: false
            maps_to: disputed_claim_ids
        returns:
          type: uuid
          name: escalation_id

      # =======================================================================
      # CONFIDENCE & STATUS
      # =======================================================================
      recalculate-confidence:
        description: Recalculate confidence score for claim(s)
        behavior: plugin
        plugin: recalculate_confidence
        args:
          - name: entity
            type: uuid
            required: false
          - name: claim
            type: uuid
            required: false
          - name: cbu
            type: uuid
            required: false
            description: Recalculate all claims for CBU
        returns:
          type: confidence_update_result

      get-verification-status:
        description: Get verification status for entity or CBU
        behavior: plugin
        plugin: get_verification_status
        args:
          - name: entity
            type: uuid
            required: false
          - name: cbu
            type: uuid
            required: false
        returns:
          type: verification_status_report
```

### 3.2 Tasks - Verification Verbs

- [ ] Create `rust/config/verbs/verify.yaml`
- [ ] Implement `verify_gleif` plugin
- [ ] Implement `verify_registry` plugin
- [ ] Implement `verify_document` plugin
- [ ] Implement `corroborate_claim` plugin
- [ ] Implement `check_consistency` plugin
- [ ] Implement `detect_patterns` plugin
- [ ] Implement `detect_evasion` plugin
- [ ] Implement `recalculate_confidence` plugin
- [ ] Create database tables for claims, challenges, escalations
- [ ] Integration tests for verification flow

---

## Part 4: End State Model (Verification-Based)

### 4.1 Verified KYC End State

**File:** `rust/config/end_states/kyc_verified.yaml`

```yaml
end_state:
  id: kyc_verified
  name: "KYC Verified"
  entity_type: cbu
  description: |
    CBU has all claims VERIFIED (not just claimed) with high confidence.
    This is an adversarial standard - would it catch a sophisticated liar?

  # ===========================================================================
  # VERIFICATION REQUIREMENTS (not just "data present")
  # ===========================================================================
  requirements:
    # =========================================================================
    # ENTITY VERIFICATION
    # =========================================================================
    - id: entity_verified
      name: "Entity Identity Verified"
      description: "Entity exists and identity verified against registry"
      evaluation:
        type: custom
        query: (verify.get-verification-status :entity $entity_id)
        condition: |
          (and (= verification_state "VERIFIED")
               (>= confidence 0.80))
      gap:
        type: unverified_entity
        severity: blocking
        message_template: "Entity identity not verified (confidence: {confidence}%)"
      resolution:
        verbs: 
          - verify.verify-against-gleif
          - verify.verify-against-registry
        auto_resolve: true
        strategy: |
          1. Check GLEIF for LEI
          2. Check relevant corporate registry
          3. Cross-reference registered name, jurisdiction, status

    # =========================================================================
    # OWNERSHIP CHAIN VERIFICATION
    # =========================================================================
    - id: ownership_claims_registered
      name: "Ownership Claims Registered"
      description: "All ownership relationships recorded as claims"
      evaluation:
        type: all_of
        query: (ubo.list-ownership :entity $entity_id)
        condition: (all? rel (claim-exists? rel))
      gap:
        type: unregistered_ownership
        severity: blocking
        message_template: "{count} ownership relationships not registered as claims"
      resolution:
        verbs: [verify.register-claim]
        auto_resolve: true
        strategy: "Register each ownership as a claim with source"

    - id: ownership_claims_verified
      name: "Ownership Claims Verified"
      description: "All ownership claims verified against independent sources"
      evaluation:
        type: all_of
        query: (verify.list-claims :entity $entity_id :type "ownership")
        condition: |
          (all? claim 
            (and (>= confidence 0.80)
                 (has-independent-source? claim)))
      gap:
        type: unverified_ownership
        severity: blocking
        message_template: |
          {count} ownership claims unverified or low confidence:
          {unverified_list}
      resolution:
        verbs:
          - verify.verify-against-gleif
          - verify.corroborate
        requires: [ownership_claims_registered]
        auto_resolve: true
        strategy: |
          For each unverified ownership:
          1. Check GLEIF for parent relationship
          2. Check corporate registry for shareholders
          3. Request share register if no registry data
          4. Cross-reference multiple sources

    - id: ownership_chain_complete
      name: "Ownership Chain Complete to UBO"
      description: "Chain reaches natural persons or valid exemption"
      evaluation:
        type: custom
        query: (ubo.evaluate-chain :entity $entity_id)
        condition: |
          (and (or (= chain_status "COMPLETE_TO_PERSONS")
                   (= chain_status "EXEMPTION_APPLIED"))
               (all-verified? chain_claims))
      gap:
        type: incomplete_ubo_chain
        severity: blocking
        message_template: |
          UBO chain incomplete or unverified at: {incomplete_entities}
      resolution:
        verbs:
          - verify.verify-against-gleif
          - ubo.apply-exemption
          - verify.challenge
        requires: [ownership_claims_verified]
        auto_resolve: true
        strategy: |
          For each incomplete path:
          1. Search GLEIF for parent
          2. Check if exemption applies (listed, regulated)
          3. If exemption, verify the exemption claim
          4. If no exemption, continue chain to natural person

    # =========================================================================
    # UBO PERSON VERIFICATION  
    # =========================================================================
    - id: ubo_persons_identified
      name: "UBO Persons Identified"
      description: "All natural person UBOs identified"
      evaluation:
        type: custom
        query: (ubo.list-natural-persons :cbu $cbu_id)
        condition: (>= (count persons) 1)
      gap:
        type: no_ubo_persons
        severity: blocking
        message_template: "No natural person UBOs identified"
      resolution:
        verbs: [entity.create-proper-person, ubo.add-ownership]
        requires: [ownership_chain_complete]
        auto_resolve: false
        strategy: "Identify natural persons from ownership chain endpoints"

    - id: ubo_persons_verified
      name: "UBO Person Identity Verified"
      description: "All UBO persons identity verified with high confidence"
      evaluation:
        type: all_of
        query: (ubo.list-natural-persons :cbu $cbu_id :include-verification true)
        condition: |
          (all? person
            (and (>= confidence 0.80)
                 (has-id-verification? person)))
      gap:
        type: unverified_ubo_persons
        severity: blocking
        message_template: |
          {count} UBO persons not identity verified: {person_list}
      resolution:
        verbs:
          - kyc.request-document
          - verify.verify-document
        requires: [ubo_persons_identified]
        auto_resolve: true
        strategy: |
          For each unverified person:
          1. Request passport or national ID
          2. Verify document authenticity
          3. Match document to claimed identity
          4. Request proof of address
          5. Verify address document

    # =========================================================================
    # CONTROL PRONG VERIFICATION
    # =========================================================================
    - id: control_persons_verified
      name: "Control Persons Verified"
      description: "Directors/officers identified and verified"
      evaluation:
        type: custom
        query: (ubo.list-control-persons :entity $cbu_id :include-verification true)
        condition: |
          (and (>= (count directors) 1)
               (all? person (>= confidence 0.75)))
      gap:
        type: unverified_control_persons
        severity: blocking
        message_template: "Control persons not identified or verified"
      resolution:
        verbs:
          - kyc.request-document
          - verify.verify-against-registry
        auto_resolve: true
        strategy: |
          1. Request register of directors from entity
          2. Verify against corporate registry
          3. For each director, verify identity

    # =========================================================================
    # ADVERSARIAL CHECKS
    # =========================================================================
    - id: no_critical_patterns
      name: "No Critical Patterns Detected"
      description: "No circular ownership, layering, or fraud indicators"
      evaluation:
        type: custom
        query: (verify.detect-patterns :cbu $cbu_id :patterns ["all"])
        condition: |
          (not-any? pattern (= risk_level "CRITICAL"))
      gap:
        type: critical_pattern_detected
        severity: blocking
        message_template: |
          Critical pattern(s) detected: {pattern_descriptions}
      resolution:
        verbs: [verify.escalate]
        auto_resolve: false
        strategy: "Critical patterns require human review and cannot proceed"

    - id: high_patterns_resolved
      name: "High-Risk Patterns Addressed"
      description: "High-risk patterns investigated and resolved"
      evaluation:
        type: custom
        query: (verify.detect-patterns :cbu $cbu_id)
        condition: |
          (all? pattern
            (or (!= risk_level "HIGH")
                (resolved? pattern)))
      gap:
        type: unresolved_high_risk_pattern
        severity: blocking
        message_template: |
          Unresolved high-risk pattern(s): {pattern_descriptions}
      resolution:
        verbs:
          - verify.challenge
          - verify.escalate
        auto_resolve: false
        strategy: |
          For each high-risk pattern:
          1. Challenge client for explanation
          2. Request supporting evidence
          3. If not satisfactorily explained, escalate

    - id: no_inconsistencies
      name: "No Unresolved Inconsistencies"
      description: "All data inconsistencies investigated and resolved"
      evaluation:
        type: custom
        query: (verify.check-consistency :cbu $cbu_id)
        condition: |
          (all? inconsistency (resolved? inconsistency))
      gap:
        type: unresolved_inconsistencies
        severity: blocking
        message_template: |
          {count} unresolved data inconsistencies
      resolution:
        verbs: [verify.challenge]
        auto_resolve: false
        strategy: |
          For each inconsistency:
          1. Challenge client to explain
          2. Determine which source is correct
          3. Update claims with resolution

    - id: no_evasion_patterns
      name: "No Evasion Behavior Detected"
      description: "Client not exhibiting evasion patterns"
      evaluation:
        type: custom
        query: (verify.detect-evasion :cbu $cbu_id)
        condition: |
          (or (empty? indicators)
              (all? ind (!= severity "HIGH")))
      gap:
        type: evasion_detected
        severity: warning
        message_template: |
          Evasion indicators detected: {indicator_descriptions}
      resolution:
        verbs: [verify.escalate]
        auto_resolve: false
        strategy: "Evasion patterns require human assessment"

    # =========================================================================
    # SCREENING VERIFICATION
    # =========================================================================
    - id: screening_complete
      name: "Screening Complete"
      description: "All parties screened with no unresolved hits"
      evaluation:
        type: all_of
        query: (screening.list :cbu $cbu_id)
        condition: |
          (and (all? party (screened? party))
               (not-any? hit (and (open? hit) (not (false-positive? hit)))))
      gap:
        type: incomplete_screening
        severity: blocking
        message_template: "Screening incomplete or unresolved hits"
      resolution:
        verbs:
          - screening.pep
          - screening.sanctions
          - screening.adverse-media
        auto_resolve: true
        strategy: "Screen all parties and resolve any hits"

    # =========================================================================
    # DOCUMENTATION
    # =========================================================================
    - id: evidence_chain_complete
      name: "Evidence Chain Complete"
      description: "All verified claims have documented evidence trail"
      evaluation:
        type: all_of
        query: (verify.list-claims :cbu $cbu_id :verified-only true)
        condition: |
          (all? claim (has-evidence-chain? claim))
      gap:
        type: missing_evidence_chain
        severity: warning
        message_template: |
          {count} claims missing complete evidence chain
      resolution:
        verbs: [kyc.link-evidence]
        auto_resolve: false
        strategy: "Document evidence source for each verified claim"

    # =========================================================================
    # CONFIDENCE THRESHOLDS
    # =========================================================================
    - id: overall_confidence
      name: "Overall Verification Confidence"
      description: "Aggregate confidence meets threshold"
      evaluation:
        type: custom
        query: (verify.get-verification-status :cbu $cbu_id)
        condition: |
          (>= overall_confidence 0.80)
      gap:
        type: low_overall_confidence
        severity: blocking
        message_template: |
          Overall confidence {confidence}% below 80% threshold
      resolution:
        verbs: [verify.corroborate]
        auto_resolve: true
        strategy: |
          1. Identify lowest-confidence claims
          2. Seek additional corroboration
          3. Recalculate overall confidence
```

### 4.2 Tasks - End State Model

- [ ] Create `rust/config/end_states/kyc_verified.yaml`
- [ ] Update all requirements to be verification-based
- [ ] Add adversarial pattern checks
- [ ] Add confidence threshold requirements
- [ ] Add evidence chain requirements
- [ ] Update gap messages to reflect verification model

---

## Part 5: Agent Strategy (Game Theory)

### 5.1 Agent Decision Framework

**File:** `rust/src/agent/strategy.rs`

```rust
//! Agent Strategy - Game Theory Decision Framework
//!
//! The agent plays an adversarial verification game.
//! Goal: Establish verified truth that would catch a sophisticated liar.

use super::gap_analysis::Gap;
use crate::verification::types::*;
use crate::verification::patterns::*;

/// Strategic decision for next action
#[derive(Debug, Clone)]
pub struct StrategicDecision {
    /// Priority (lower = do first)
    pub priority: u32,
    /// The action to take
    pub action: StrategicAction,
    /// Reasoning
    pub rationale: String,
    /// Expected outcome
    pub expected_outcome: String,
    /// Risk if not done
    pub risk_if_skipped: RiskLevel,
}

#[derive(Debug, Clone)]
pub enum StrategicAction {
    /// Verify a claim against independent source
    Verify { claim_id: Uuid, source: SourceType },
    /// Seek corroboration from multiple sources
    Corroborate { claim_id: Uuid, sources: Vec<SourceType> },
    /// Challenge an inconsistency or suspicious element
    Challenge { entity_id: Uuid, challenge_type: String, questions: Vec<String> },
    /// Request specific evidence
    RequestEvidence { entity_id: Uuid, evidence_type: String },
    /// Run pattern detection
    DetectPatterns { cbu_id: Uuid },
    /// Escalate to human
    Escalate { cbu_id: Uuid, reason: String },
    /// Recalculate confidence
    RecalculateConfidence { scope: Uuid },
}

/// Agent strategy engine
pub struct StrategyEngine {
    /// Risk tolerance (lower = more aggressive verification)
    risk_tolerance: f32,
}

impl StrategyEngine {
    pub fn new(risk_tolerance: f32) -> Self {
        Self { risk_tolerance }
    }

    /// Determine strategic priorities
    pub fn plan_strategy(
        &self,
        gaps: &[Gap],
        claims: &[Claim],
        patterns: &[DetectedPattern],
    ) -> Vec<StrategicDecision> {
        let mut decisions = Vec::new();

        // PRIORITY 1: Address critical patterns immediately
        for pattern in patterns.iter().filter(|p| p.risk_level == RiskLevel::Critical) {
            decisions.push(StrategicDecision {
                priority: 0,
                action: StrategicAction::Escalate {
                    cbu_id: pattern.entities[0], // Simplification
                    reason: format!("Critical pattern: {}", pattern.description),
                },
                rationale: "Critical patterns require immediate human review".into(),
                expected_outcome: "Human will assess and determine next steps".into(),
                risk_if_skipped: RiskLevel::Critical,
            });
        }

        // PRIORITY 2: Verify claims that are still CLAIMED (not verified)
        let unverified_claims: Vec<&Claim> = claims.iter()
            .filter(|c| c.verification.status == VerificationState::Claimed)
            .collect();

        for claim in unverified_claims {
            // Prioritize ownership and identity claims
            let priority = match claim.claim_type {
                ClaimType::Ownership { .. } => 1,
                ClaimType::PersonIdentity => 1,
                ClaimType::Control { .. } => 2,
                ClaimType::RegulatoryStatus => 2,
                _ => 3,
            };

            // Determine best verification source
            let source = self.best_verification_source(&claim.claim_type);

            decisions.push(StrategicDecision {
                priority,
                action: StrategicAction::Verify {
                    claim_id: claim.id,
                    source,
                },
                rationale: format!(
                    "Claim '{}' is unverified (source: {:?}, confidence: {:.0}%)",
                    claim.content.description,
                    claim.source.source_type,
                    claim.confidence * 100.0
                ),
                expected_outcome: "Claim verified against independent source".into(),
                risk_if_skipped: if priority <= 1 { RiskLevel::High } else { RiskLevel::Medium },
            });
        }

        // PRIORITY 3: Address low-confidence claims
        let low_confidence: Vec<&Claim> = claims.iter()
            .filter(|c| c.confidence < 0.60 && c.verification.status != VerificationState::Refuted)
            .collect();

        for claim in low_confidence {
            decisions.push(StrategicDecision {
                priority: 4,
                action: StrategicAction::Corroborate {
                    claim_id: claim.id,
                    sources: vec![SourceType::Gleif, SourceType::GovernmentRegistry],
                },
                rationale: format!(
                    "Low confidence ({:.0}%) on claim: {}",
                    claim.confidence * 100.0,
                    claim.content.description
                ),
                expected_outcome: "Confidence increased through corroboration".into(),
                risk_if_skipped: RiskLevel::Medium,
            });
        }

        // PRIORITY 4: Resolve inconsistencies
        for claim in claims.iter().filter(|c| !c.inconsistencies.is_empty()) {
            for inconsistency in &claim.inconsistencies {
                if !inconsistency.resolved {
                    decisions.push(StrategicDecision {
                        priority: 3,
                        action: StrategicAction::Challenge {
                            entity_id: claim.subject_id,
                            challenge_type: "inconsistency".into(),
                            questions: vec![
                                format!(
                                    "Source A says '{}', Source B says '{}'. Which is correct?",
                                    inconsistency.value_a,
                                    inconsistency.value_b
                                ),
                            ],
                        },
                        rationale: format!("Inconsistency: {}", inconsistency.description),
                        expected_outcome: "Inconsistency explained and resolved".into(),
                        risk_if_skipped: match inconsistency.severity {
                            InconsistencySeverity::Critical => RiskLevel::Critical,
                            InconsistencySeverity::Serious => RiskLevel::High,
                            InconsistencySeverity::Moderate => RiskLevel::Medium,
                            InconsistencySeverity::Minor => RiskLevel::Low,
                        },
                    });
                }
            }
        }

        // PRIORITY 5: Investigate high-risk patterns
        for pattern in patterns.iter().filter(|p| p.risk_level == RiskLevel::High) {
            decisions.push(StrategicDecision {
                priority: 5,
                action: StrategicAction::Challenge {
                    entity_id: pattern.entities[0],
                    challenge_type: format!("{:?}", pattern.pattern_type),
                    questions: self.questions_for_pattern(pattern),
                },
                rationale: format!("High-risk pattern: {}", pattern.description),
                expected_outcome: "Pattern explained or escalated".into(),
                risk_if_skipped: RiskLevel::High,
            });
        }

        // Sort by priority
        decisions.sort_by_key(|d| d.priority);

        decisions
    }

    fn best_verification_source(&self, claim_type: &ClaimType) -> SourceType {
        match claim_type {
            ClaimType::EntityExists => SourceType::GovernmentRegistry,
            ClaimType::Ownership { .. } => SourceType::Gleif,
            ClaimType::Control { .. } => SourceType::GovernmentRegistry,
            ClaimType::PersonIdentity => SourceType::GovernmentRegistry,
            ClaimType::Jurisdiction => SourceType::Gleif,
            ClaimType::RegulatoryStatus => SourceType::ExchangeListing,
            ClaimType::DocumentAuthenticity => SourceType::NotarizedDocument,
        }
    }

    fn questions_for_pattern(&self, pattern: &DetectedPattern) -> Vec<String> {
        match pattern.pattern_type {
            PatternType::LayeringStructure => vec![
                "What is the business purpose for this ownership structure?".into(),
                "Why are intermediate holding entities necessary?".into(),
                "Provide documentation for each layer's business activity".into(),
            ],
            PatternType::OpacityJurisdictionUsage => vec![
                "Why are entities domiciled in these jurisdictions?".into(),
                "Provide source of funds documentation".into(),
                "Provide regulatory status in each jurisdiction".into(),
            ],
            PatternType::NomineeUsage => vec![
                "Confirm whether nominee arrangements are in place".into(),
                "If nominees used, identify the underlying beneficial owner".into(),
                "Provide documentation of nominee arrangement".into(),
            ],
            PatternType::RegistryMismatch => vec![
                format!(
                    "Registry shows different information than provided. Please explain: {}",
                    pattern.description
                ),
            ],
            _ => vec![
                format!("Please explain the following: {}", pattern.description),
            ],
        }
    }
}

/// The core game theory principles
pub mod principles {
    /// NEVER accept client claims at face value
    pub const DISTRUST_BY_DEFAULT: &str = 
        "Every claim starts as UNVERIFIED regardless of source. \
         Client-provided information has low base confidence.";

    /// Always seek independent verification
    pub const INDEPENDENT_VERIFICATION: &str = 
        "Verify every material claim against at least one independent source. \
         GLEIF, government registries, and exchange listings are preferred.";

    /// Multiple sources increase confidence
    pub const CORROBORATION: &str = 
        "Two independent sources agreeing significantly increases confidence. \
         Conflicting sources trigger investigation.";

    /// Inconsistencies are red flags
    pub const INCONSISTENCY_ALERT: &str = 
        "Any inconsistency between sources must be resolved. \
         Unexplained inconsistencies block verification.";

    /// Patterns reveal intent
    pub const PATTERN_ANALYSIS: &str = 
        "Structural and behavioral patterns can indicate deception. \
         Circular ownership, layering, nominee usage require investigation.";

    /// The test: would this catch a liar?
    pub const ADVERSARIAL_STANDARD: &str = 
        "Every verification decision should answer: \
         'Would this process catch a sophisticated liar trying to hide true ownership?'";
}
```

### 5.2 Agent System Prompt

**File:** `rust/config/agent/adversarial_agent_prompt.md`

```markdown
# Adversarial KYC Agent System Prompt

You are a KYC verification agent engaged in an adversarial verification game.

## The Game

**You (Agent)**: Establish VERIFIED truth about beneficial ownership
**Client**: Achieve KYC clearance (may be honest OR deceptive)

**Your Win**: All claims verified to high confidence, would catch a sophisticated liar
**Your Loss**: Accept unverified claims (even from honest client = systemic failure)

## Core Principles

### 1. DISTRUST BY DEFAULT
- Client claims are UNVERIFIED until proven
- Client documents have low base confidence (40-60%)
- "Trust but verify" means VERIFY EVERYTHING

### 2. INDEPENDENT VERIFICATION
Every material claim needs independent confirmation:
- GLEIF (ownership, LEI, jurisdiction)
- Government registries (incorporation, directors)
- Exchange listings (public company status)
- Regulatory filings (fund registration)

### 3. CORROBORATION STRENGTHENS
- One source = weak
- Two sources agreeing = strong
- Three+ sources = very high confidence
- Conflicting sources = INVESTIGATE

### 4. INCONSISTENCIES ARE RED FLAGS
Never ignore:
- Client says X, registry says Y
- Document dates don't align
- Jurisdiction mismatches
- Story changes when challenged

### 5. PATTERNS REVEAL INTENT
Watch for:
- Circular ownership (A→B→C→A)
- Deep layering with single owners
- Multiple opacity jurisdictions
- Nominee usage without explanation
- Selective document delays

## Verification Verbs

```lisp
;; Register what client claims
(verify.register-claim :entity @e :claim-type "ownership" 
                       :source-type "client_uncertified" ...)

;; Verify against independent source
(verify.verify-against-gleif :entity @e)
(verify.verify-against-registry :entity @e :registry "companies_house_uk")

;; Seek corroboration
(verify.corroborate :claim @c :sources ["gleif" "registry"])

;; Check for inconsistencies
(verify.check-consistency :cbu @cbu)

;; Detect adversarial patterns
(verify.detect-patterns :cbu @cbu)
(verify.detect-evasion :cbu @cbu)

;; Challenge when needed
(verify.challenge :entity @e :challenge-type "inconsistency"
                  :questions ["Registry shows X, you claim Y. Explain."])

;; Escalate when necessary
(verify.escalate :cbu @cbu :reason "Critical pattern" :risk-level "critical")
```

## Decision Process

For every claim:

1. **Register** - Record claim with source type
2. **Assess** - What is the base confidence?
3. **Verify** - Can we check an independent source?
4. **Corroborate** - Can we find a second source?
5. **Challenge** - Are there inconsistencies to resolve?
6. **Decide** - Is confidence above threshold?

## Confidence Thresholds

| Score | State | Action |
|-------|-------|--------|
| ≥80% | VERIFIED | Can proceed |
| 60-79% | PROVISIONAL | Need more evidence |
| 40-59% | UNVERIFIED | Actively challenge |
| <40% | SUSPECT | Escalate immediately |

## Red Lines (Always Escalate)

- Circular ownership detected
- Registry data contradicts client
- Critical pattern detected
- Client refuses to explain structure
- Document appears altered
- Multiple high-risk indicators

## The Test

Before approving anything, ask:

> "If this client were a sophisticated money launderer trying to 
> hide true beneficial ownership, would this verification process 
> catch them?"

If the answer is "maybe not", the verification is insufficient.

## Example Interaction

User: "Verify ownership for FundCo Ltd"

Agent: Let me verify the ownership claims.

```lisp
;; First, check what we have
(verify.list-claims :entity @fundco :type "ownership")
```

I see 3 ownership claims, all from client documents (confidence: 40-50%).

Let me verify against independent sources:

```lisp
;; Check GLEIF
(verify.verify-against-gleif :entity @fundco)
```

GLEIF shows FundCo's parent is HoldingCo (LEI: 5493...) with 100% ownership.
This matches claim #1. ✓ Confidence now 85%.

But claim #2 says HoldingCo is owned by "ParentCorp BVI".
GLEIF shows HoldingCo's parent is "ParentCorp Ltd (UK)".

**INCONSISTENCY DETECTED**

```lisp
(verify.challenge :entity @holdingco 
                  :challenge-type "registry_mismatch"
                  :questions ["GLEIF shows UK parent, you claimed BVI parent. 
                              Which is correct and why the discrepancy?"])
```

I cannot proceed until this inconsistency is resolved. The discrepancy 
between a BVI and UK parent is significant - different regulatory regimes,
different transparency requirements.

Current status: **BLOCKED** pending inconsistency resolution.
```

---

### 5.3 Tasks - Agent Strategy

- [ ] Create `rust/src/agent/strategy.rs`
- [ ] Implement `StrategyEngine`
- [ ] Implement strategic decision prioritization
- [ ] Implement pattern-specific question generation
- [ ] Create `rust/config/agent/adversarial_agent_prompt.md`
- [ ] Integrate strategy engine with agent executor
- [ ] Test agent with simulated adversarial scenarios

---

## Part 6: Feedback & Learning (Updated)

### 6.1 Telemetry for Adversarial Model

Track not just execution, but **verification effectiveness**:

```rust
/// Extended telemetry for adversarial model
#[derive(Debug, Clone, Serialize)]
pub struct VerificationTelemetry {
    /// Execution record
    pub execution: ExecutionRecord,
    
    /// Verification effectiveness
    pub verification_stats: VerificationStats,
    
    /// Pattern detection effectiveness
    pub pattern_stats: PatternStats,
    
    /// Adversarial indicators
    pub adversarial_indicators: Vec<AdversarialIndicator>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerificationStats {
    /// Claims started as CLAIMED
    pub claims_received: usize,
    /// Claims verified to ≥80%
    pub claims_verified: usize,
    /// Claims that failed verification
    pub claims_refuted: usize,
    /// Inconsistencies detected
    pub inconsistencies_found: usize,
    /// Inconsistencies resolved
    pub inconsistencies_resolved: usize,
    /// Average confidence improvement
    pub avg_confidence_improvement: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatternStats {
    /// Patterns detected
    pub patterns_detected: usize,
    /// By type
    pub by_type: HashMap<String, usize>,
    /// Patterns that led to escalation
    pub patterns_escalated: usize,
    /// Patterns satisfactorily explained
    pub patterns_cleared: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdversarialIndicator {
    /// What was detected
    pub indicator: String,
    /// Outcome
    pub outcome: AdversarialOutcome,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum AdversarialOutcome {
    /// False alarm - legitimate explanation
    FalsePositive,
    /// Real issue - properly escalated
    TruePositive,
    /// Missed - detected later
    FalseNegative,
    /// Unknown - pending resolution
    Pending,
}
```

### 6.2 Learning Focus

The model learns:

1. **Which patterns predict real problems** vs false positives
2. **Which verification sources are most reliable** for each claim type
3. **Which challenge questions** get useful responses
4. **Evasion signatures** - what behavior preceded later-discovered fraud
5. **Jurisdiction-specific** verification requirements

---

## Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE ADVERSARIAL MODEL                                                      │
│                                                                             │
│  NOT: "Collect data to complete checklist"                                 │
│  BUT: "Verify claims to catch a sophisticated liar"                        │
│                                                                             │
│  EVERY CLAIM:                                                               │
│  1. Registered with source                                                 │
│  2. Assigned base confidence                                               │
│  3. Verified against independent source                                    │
│  4. Corroborated if possible                                               │
│  5. Checked for inconsistencies                                            │
│  6. Patterns analyzed                                                      │
│  7. Confidence recalculated                                                │
│  8. Either VERIFIED (≥80%) or CHALLENGED                                   │
│                                                                             │
│  THE STANDARD:                                                              │
│  "Would this process catch a sophisticated liar?"                          │
│                                                                             │
│  If not → verification is insufficient                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Order

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  PHASE 1: Verification Types (2 days)                                       │
│  • Claim, Evidence, Confidence models                                       │
│  • Confidence calculator                                                   │
│  • Database tables                                                         │
│                                                                             │
│  PHASE 2: Adversarial Detection (2 days)                                    │
│  • Pattern detector                                                        │
│  • Evasion detector                                                        │
│  • Integration with graph                                                  │
│                                                                             │
│  PHASE 3: Verification Verbs (2 days)                                       │
│  • verify.* verb implementations                                           │
│  • GLEIF verification                                                      │
│  • Registry verification                                                   │
│                                                                             │
│  PHASE 4: End State & Strategy (2 days)                                     │
│  • kyc_verified end state                                                  │
│  • Strategy engine                                                         │
│  • Agent prompt                                                            │
│                                                                             │
│  PHASE 5: Integration & Testing (2 days)                                    │
│  • Agent orchestration                                                     │
│  • Adversarial test scenarios                                              │
│  • Telemetry                                                               │
│                                                                             │
│  TOTAL: ~10 days                                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Success Criteria

- [ ] Every claim has confidence score
- [ ] Verification against GLEIF works
- [ ] Pattern detection finds circular ownership
- [ ] Inconsistencies are flagged
- [ ] Agent challenges unverified claims
- [ ] Agent escalates critical patterns
- [ ] Confidence thresholds enforced
- [ ] Would catch a simulated liar in test scenarios

---

*The game: Verify truth. Catch lies. Never accept unverified claims.*
