# TODO: UBO Verification Test Harness

## ⛔ MANDATORY FIRST STEP

**Read these files before starting:**
- `/TODO-ADAPTIVE-AGENT-MODEL.md` - The adversarial model being tested
- `/rust/src/verification/types.rs` - Claim and confidence types
- `/rust/src/verification/patterns.rs` - Pattern detection
- `/rust/config/end_states/kyc_verified.yaml` - End state requirements

---

## Overview

A test harness to animate UBO verification cases from initial allegation through 
to KYC decision. Tests both **honest client** and **adversarial client** scenarios 
to validate the game theory model.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TEST HARNESS GOAL                                                          │
│                                                                             │
│  Simulate complete UBO verification lifecycle:                             │
│                                                                             │
│  1. ALLEGATION      Client claims ownership structure                      │
│  2. REGISTRATION    Claims registered with source types                    │
│  3. VERIFICATION    Agent verifies against independent sources             │
│  4. DETECTION       Patterns and inconsistencies detected                  │
│  5. CHALLENGE       Agent challenges suspicious elements                   │
│  6. RESOLUTION      Client responds (truthfully or not)                    │
│  7. DECISION        KYC approved, rejected, or escalated                   │
│                                                                             │
│  Test both:                                                                 │
│  • HONEST CLIENT → Should be verified efficiently                          │
│  • ADVERSARIAL CLIENT → Should be caught/blocked                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Test Scenario Framework

### 1.1 Scenario Definition

**File:** `rust/tests/harness/scenario.rs`

```rust
//! Test Scenario Definition
//!
//! Defines a complete UBO case with expected outcomes.

use uuid::Uuid;
use std::collections::HashMap;

/// A complete test scenario
#[derive(Debug, Clone)]
pub struct TestScenario {
    /// Scenario identifier
    pub id: String,
    /// Human readable name
    pub name: String,
    /// Description of what's being tested
    pub description: String,
    
    /// The client type (honest or adversarial)
    pub client_type: ClientType,
    
    /// Initial structure claimed by client
    pub claimed_structure: ClaimedStructure,
    
    /// What the "truth" actually is (for simulation)
    pub ground_truth: GroundTruth,
    
    /// Simulated external data sources
    pub external_sources: ExternalSources,
    
    /// How client responds to challenges
    pub client_behavior: ClientBehavior,
    
    /// Expected outcome
    pub expected_outcome: ExpectedOutcome,
    
    /// Specific assertions to check
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientType {
    /// Provides truthful information
    Honest,
    /// Attempts to deceive
    Adversarial,
    /// Honest but disorganized/slow
    Uncooperative,
}

/// What the client claims the structure is
#[derive(Debug, Clone)]
pub struct ClaimedStructure {
    /// The CBU being onboarded
    pub cbu: EntityClaim,
    /// Ownership chain as claimed
    pub ownership_chain: Vec<OwnershipClaim>,
    /// Control persons claimed
    pub control_persons: Vec<ControlPersonClaim>,
    /// Documents provided
    pub documents: Vec<DocumentClaim>,
}

#[derive(Debug, Clone)]
pub struct EntityClaim {
    pub name: String,
    pub entity_type: String,
    pub jurisdiction: String,
    pub lei: Option<String>,
    pub registration_number: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OwnershipClaim {
    pub owner: String,  // Entity name
    pub owned: String,  // Entity name
    pub percentage: f32,
    pub source_document: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ControlPersonClaim {
    pub person_name: String,
    pub role: String,
    pub entity: String,  // Entity they control
}

#[derive(Debug, Clone)]
pub struct DocumentClaim {
    pub document_type: String,
    pub entity: String,
    pub is_authentic: bool,  // For simulation - is it real?
    pub is_current: bool,    // Not expired?
    pub content_matches_claim: bool,
}

/// The actual truth (for adversarial scenarios)
#[derive(Debug, Clone)]
pub struct GroundTruth {
    /// True ownership structure (may differ from claimed)
    pub true_ownership: Vec<OwnershipClaim>,
    /// True control persons
    pub true_control: Vec<ControlPersonClaim>,
    /// Hidden entities not disclosed
    pub hidden_entities: Vec<EntityClaim>,
    /// True UBO (natural person)
    pub true_ubo: Option<String>,
}

/// Simulated external data sources
#[derive(Debug, Clone)]
pub struct ExternalSources {
    /// What GLEIF would return
    pub gleif_data: HashMap<String, GleifRecord>,
    /// What corporate registries would return
    pub registry_data: HashMap<String, RegistryRecord>,
    /// Screening results
    pub screening_results: HashMap<String, ScreeningResult>,
}

#[derive(Debug, Clone)]
pub struct GleifRecord {
    pub lei: String,
    pub legal_name: String,
    pub jurisdiction: String,
    pub parent_lei: Option<String>,
    pub parent_name: Option<String>,
    pub ultimate_parent_lei: Option<String>,
    pub entity_status: String,
}

#[derive(Debug, Clone)]
pub struct RegistryRecord {
    pub name: String,
    pub registration_number: String,
    pub jurisdiction: String,
    pub status: String,
    pub directors: Vec<String>,
    pub shareholders: Vec<(String, f32)>,  // (name, percentage)
}

#[derive(Debug, Clone)]
pub struct ScreeningResult {
    pub entity_name: String,
    pub pep_hit: bool,
    pub pep_details: Option<String>,
    pub sanctions_hit: bool,
    pub sanctions_details: Option<String>,
    pub adverse_media_hit: bool,
    pub adverse_media_details: Option<String>,
}

/// How client behaves when challenged
#[derive(Debug, Clone)]
pub struct ClientBehavior {
    /// Response delay in simulated days
    pub response_delay: ResponseDelay,
    /// How they respond to challenges
    pub challenge_responses: HashMap<String, ChallengeResponse>,
    /// Documents they refuse to provide
    pub refused_documents: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ResponseDelay {
    /// Responds promptly (1-3 days)
    Prompt,
    /// Normal (5-10 days)
    Normal,
    /// Slow (15-30 days)
    Slow,
    /// Selective (fast on some, slow on others)
    Selective { fast_types: Vec<String>, slow_types: Vec<String> },
}

#[derive(Debug, Clone)]
pub enum ChallengeResponse {
    /// Provides truthful explanation
    Truthful { explanation: String },
    /// Provides false explanation
    Deceptive { false_explanation: String },
    /// Changes story
    Inconsistent { new_claim: String },
    /// Refuses to answer
    Refuses,
    /// Delays indefinitely
    Evades,
}

/// Expected outcome of the scenario
#[derive(Debug, Clone)]
pub struct ExpectedOutcome {
    /// Final decision
    pub decision: Decision,
    /// Expected confidence level at end
    pub min_confidence: Option<f32>,
    /// Patterns that should be detected
    pub patterns_detected: Vec<String>,
    /// Should this be escalated?
    pub escalated: bool,
    /// Reason for outcome
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// KYC approved - verified
    Approved,
    /// KYC rejected - failed verification
    Rejected,
    /// Escalated to human - needs review
    Escalated,
    /// Blocked - critical issue
    Blocked,
    /// Timed out - client unresponsive
    TimedOut,
}

/// Specific things to assert during/after test
#[derive(Debug, Clone)]
pub struct Assertion {
    /// When to check (after which phase)
    pub check_after: Phase,
    /// What to assert
    pub assertion_type: AssertionType,
    /// Expected value
    pub expected: String,
}

#[derive(Debug, Clone, Copy)]
pub enum Phase {
    Registration,
    Verification,
    PatternDetection,
    Challenge,
    Decision,
}

#[derive(Debug, Clone)]
pub enum AssertionType {
    /// Claim has specific confidence
    ClaimConfidence { claim_type: String, min: f32, max: f32 },
    /// Pattern was detected
    PatternDetected { pattern_type: String },
    /// Inconsistency was found
    InconsistencyFound { description: String },
    /// Challenge was issued
    ChallengeIssued { challenge_type: String },
    /// Escalation triggered
    EscalationTriggered,
    /// Specific verb was executed
    VerbExecuted { verb: String },
}
```

### 1.2 Tasks - Scenario Framework

- [ ] Create `rust/tests/harness/` directory
- [ ] Create `rust/tests/harness/scenario.rs`
- [ ] Implement all scenario types
- [ ] Create scenario builder for easy test creation

---

## Part 2: Simulated Environment

### 2.1 Mock External Sources

**File:** `rust/tests/harness/mock_sources.rs`

```rust
//! Mock External Data Sources
//!
//! Simulates GLEIF, registries, screening providers for testing.

use super::scenario::*;
use async_trait::async_trait;

/// Mock GLEIF API
pub struct MockGleif {
    data: HashMap<String, GleifRecord>,
}

impl MockGleif {
    pub fn from_scenario(sources: &ExternalSources) -> Self {
        Self {
            data: sources.gleif_data.clone(),
        }
    }

    /// Lookup by LEI
    pub async fn lookup_lei(&self, lei: &str) -> Option<GleifRecord> {
        self.data.get(lei).cloned()
    }

    /// Search by name
    pub async fn search_name(&self, name: &str) -> Vec<GleifRecord> {
        self.data.values()
            .filter(|r| r.legal_name.to_lowercase().contains(&name.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Get parent chain
    pub async fn get_parent_chain(&self, lei: &str) -> Vec<GleifRecord> {
        let mut chain = Vec::new();
        let mut current = lei.to_string();
        
        while let Some(record) = self.data.get(&current) {
            chain.push(record.clone());
            match &record.parent_lei {
                Some(parent) => current = parent.clone(),
                None => break,
            }
        }
        
        chain
    }
}

/// Mock Corporate Registry
pub struct MockRegistry {
    registries: HashMap<String, HashMap<String, RegistryRecord>>,
}

impl MockRegistry {
    pub fn from_scenario(sources: &ExternalSources) -> Self {
        // Group by jurisdiction
        let mut registries: HashMap<String, HashMap<String, RegistryRecord>> = HashMap::new();
        
        for (key, record) in &sources.registry_data {
            registries
                .entry(record.jurisdiction.clone())
                .or_default()
                .insert(key.clone(), record.clone());
        }
        
        Self { registries }
    }

    /// Lookup in specific registry
    pub async fn lookup(
        &self, 
        jurisdiction: &str, 
        registration_number: &str
    ) -> Option<RegistryRecord> {
        self.registries
            .get(jurisdiction)?
            .get(registration_number)
            .cloned()
    }

    /// Search by name in jurisdiction
    pub async fn search_name(
        &self,
        jurisdiction: &str,
        name: &str,
    ) -> Vec<RegistryRecord> {
        self.registries
            .get(jurisdiction)
            .map(|reg| {
                reg.values()
                    .filter(|r| r.name.to_lowercase().contains(&name.to_lowercase()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Mock Screening Provider
pub struct MockScreening {
    results: HashMap<String, ScreeningResult>,
}

impl MockScreening {
    pub fn from_scenario(sources: &ExternalSources) -> Self {
        Self {
            results: sources.screening_results.clone(),
        }
    }

    /// Run PEP screening
    pub async fn screen_pep(&self, name: &str) -> ScreeningResult {
        self.results.get(name).cloned().unwrap_or(ScreeningResult {
            entity_name: name.to_string(),
            pep_hit: false,
            pep_details: None,
            sanctions_hit: false,
            sanctions_details: None,
            adverse_media_hit: false,
            adverse_media_details: None,
        })
    }

    /// Run sanctions screening
    pub async fn screen_sanctions(&self, name: &str) -> ScreeningResult {
        self.screen_pep(name).await  // Same mock for simplicity
    }
}

/// Mock Client (simulates client responses)
pub struct MockClient {
    behavior: ClientBehavior,
    claimed_structure: ClaimedStructure,
}

impl MockClient {
    pub fn from_scenario(scenario: &TestScenario) -> Self {
        Self {
            behavior: scenario.client_behavior.clone(),
            claimed_structure: scenario.claimed_structure.clone(),
        }
    }

    /// Client provides a document
    pub async fn request_document(
        &self,
        document_type: &str,
        entity: &str,
    ) -> DocumentResponse {
        // Check if client refuses
        if self.behavior.refused_documents.contains(&document_type.to_string()) {
            return DocumentResponse::Refused {
                reason: "Client declined to provide".into(),
            };
        }

        // Find document in claims
        let doc = self.claimed_structure.documents.iter()
            .find(|d| d.document_type == document_type && d.entity == entity);

        match doc {
            Some(d) => DocumentResponse::Provided {
                document: d.clone(),
                delay_days: self.get_delay(document_type),
            },
            None => DocumentResponse::NotAvailable {
                reason: "Document not available".into(),
            },
        }
    }

    /// Client responds to challenge
    pub async fn respond_to_challenge(
        &self,
        challenge_type: &str,
    ) -> ChallengeResponse {
        self.behavior.challenge_responses
            .get(challenge_type)
            .cloned()
            .unwrap_or(ChallengeResponse::Refuses)
    }

    fn get_delay(&self, document_type: &str) -> u32 {
        match &self.behavior.response_delay {
            ResponseDelay::Prompt => 2,
            ResponseDelay::Normal => 7,
            ResponseDelay::Slow => 21,
            ResponseDelay::Selective { fast_types, slow_types } => {
                if fast_types.contains(&document_type.to_string()) {
                    2
                } else if slow_types.contains(&document_type.to_string()) {
                    30
                } else {
                    7
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DocumentResponse {
    Provided { document: DocumentClaim, delay_days: u32 },
    Refused { reason: String },
    NotAvailable { reason: String },
}
```

### 2.2 Tasks - Mock Sources

- [ ] Create `rust/tests/harness/mock_sources.rs`
- [ ] Implement `MockGleif`
- [ ] Implement `MockRegistry`
- [ ] Implement `MockScreening`
- [ ] Implement `MockClient`
- [ ] Unit tests for mocks

---

## Part 3: Test Runner

### 3.1 Scenario Runner

**File:** `rust/tests/harness/runner.rs`

```rust
//! Test Scenario Runner
//!
//! Executes scenarios and validates outcomes.

use super::scenario::*;
use super::mock_sources::*;
use std::collections::HashMap;

/// Runs a test scenario end-to-end
pub struct ScenarioRunner {
    /// Mock external sources
    gleif: MockGleif,
    registry: MockRegistry,
    screening: MockScreening,
    client: MockClient,
    
    /// Scenario being run
    scenario: TestScenario,
    
    /// Execution log
    log: ExecutionLog,
    
    /// Current state
    state: VerificationState,
}

#[derive(Debug, Default)]
pub struct ExecutionLog {
    pub events: Vec<LogEvent>,
    pub verbs_executed: Vec<String>,
    pub claims_registered: Vec<ClaimLog>,
    pub patterns_detected: Vec<String>,
    pub challenges_issued: Vec<ChallengeLog>,
    pub escalations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LogEvent {
    pub timestamp: u64,  // Simulated time
    pub phase: Phase,
    pub event_type: String,
    pub details: String,
}

#[derive(Debug, Clone)]
pub struct ClaimLog {
    pub claim_type: String,
    pub subject: String,
    pub initial_confidence: f32,
    pub final_confidence: f32,
    pub verification_state: String,
}

#[derive(Debug, Clone)]
pub struct ChallengeLog {
    pub challenge_type: String,
    pub entity: String,
    pub response: String,
}

#[derive(Debug, Default)]
pub struct VerificationState {
    pub claims: HashMap<String, ClaimState>,
    pub overall_confidence: f32,
    pub patterns_found: Vec<PatternState>,
    pub inconsistencies: Vec<InconsistencyState>,
    pub decision: Option<Decision>,
}

#[derive(Debug, Clone)]
pub struct ClaimState {
    pub claim_type: String,
    pub subject: String,
    pub confidence: f32,
    pub verified: bool,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PatternState {
    pub pattern_type: String,
    pub risk_level: String,
    pub resolved: bool,
}

#[derive(Debug, Clone)]
pub struct InconsistencyState {
    pub description: String,
    pub resolved: bool,
}

impl ScenarioRunner {
    pub fn new(scenario: TestScenario) -> Self {
        let gleif = MockGleif::from_scenario(&scenario.external_sources);
        let registry = MockRegistry::from_scenario(&scenario.external_sources);
        let screening = MockScreening::from_scenario(&scenario.external_sources);
        let client = MockClient::from_scenario(&scenario);

        Self {
            gleif,
            registry,
            screening,
            client,
            scenario,
            log: ExecutionLog::default(),
            state: VerificationState::default(),
        }
    }

    /// Run the complete scenario
    pub async fn run(&mut self) -> ScenarioResult {
        println!("═══════════════════════════════════════════════════════════════");
        println!("SCENARIO: {}", self.scenario.name);
        println!("TYPE: {:?}", self.scenario.client_type);
        println!("═══════════════════════════════════════════════════════════════\n");

        // Phase 1: Registration
        self.phase_registration().await;
        self.check_assertions(Phase::Registration);

        // Phase 2: Verification
        self.phase_verification().await;
        self.check_assertions(Phase::Verification);

        // Phase 3: Pattern Detection
        self.phase_pattern_detection().await;
        self.check_assertions(Phase::PatternDetection);

        // Phase 4: Challenge (if needed)
        self.phase_challenge().await;
        self.check_assertions(Phase::Challenge);

        // Phase 5: Decision
        self.phase_decision().await;
        self.check_assertions(Phase::Decision);

        // Validate outcome
        self.validate_outcome()
    }

    async fn phase_registration(&mut self) {
        println!("┌─ PHASE 1: REGISTRATION ─────────────────────────────────────┐");
        
        // Register CBU entity claim
        let cbu = &self.scenario.claimed_structure.cbu;
        self.register_claim("entity_exists", &cbu.name, 0.40);
        
        // Register ownership claims
        for ownership in &self.scenario.claimed_structure.ownership_chain {
            let confidence = if ownership.source_document.is_some() { 0.50 } else { 0.30 };
            self.register_claim(
                &format!("ownership:{}:{}", ownership.owner, ownership.owned),
                &ownership.owned,
                confidence,
            );
        }

        // Register control person claims
        for control in &self.scenario.claimed_structure.control_persons {
            self.register_claim(
                &format!("control:{}:{}", control.person_name, control.entity),
                &control.entity,
                0.40,
            );
        }

        println!("│ Registered {} claims", self.state.claims.len());
        println!("└──────────────────────────────────────────────────────────────┘\n");
    }

    async fn phase_verification(&mut self) {
        println!("┌─ PHASE 2: VERIFICATION ─────────────────────────────────────┐");

        // Verify each entity against GLEIF
        for ownership in &self.scenario.claimed_structure.ownership_chain {
            // Check owner in GLEIF
            if let Some(gleif) = self.gleif.search_name(&ownership.owner).await.first() {
                let claim_key = format!("ownership:{}:{}", ownership.owner, ownership.owned);
                self.verify_claim(&claim_key, "GLEIF", gleif.parent_name.is_some());
                println!("│ ✓ GLEIF verified: {}", ownership.owner);
            } else {
                println!("│ ✗ GLEIF not found: {}", ownership.owner);
            }
        }

        // Cross-reference with registries
        let cbu = &self.scenario.claimed_structure.cbu;
        if let Some(reg_num) = &cbu.registration_number {
            if let Some(registry) = self.registry.lookup(&cbu.jurisdiction, reg_num).await {
                self.verify_claim("entity_exists", "Registry", true);
                println!("│ ✓ Registry verified: {}", cbu.name);
                
                // Check for mismatches
                if registry.name.to_lowercase() != cbu.name.to_lowercase() {
                    self.add_inconsistency(&format!(
                        "Registry name '{}' != claimed name '{}'",
                        registry.name, cbu.name
                    ));
                }
            }
        }

        // Calculate overall confidence
        self.recalculate_confidence();
        
        println!("│ Overall confidence: {:.0}%", self.state.overall_confidence * 100.0);
        println!("└──────────────────────────────────────────────────────────────┘\n");
    }

    async fn phase_pattern_detection(&mut self) {
        println!("┌─ PHASE 3: PATTERN DETECTION ────────────────────────────────┐");

        // Check for circular ownership
        if self.detect_circular_ownership() {
            self.state.patterns_found.push(PatternState {
                pattern_type: "circular_ownership".into(),
                risk_level: "CRITICAL".into(),
                resolved: false,
            });
            println!("│ ⚠ CRITICAL: Circular ownership detected!");
        }

        // Check for layering
        let chain_depth = self.scenario.claimed_structure.ownership_chain.len();
        if chain_depth > 4 {
            self.state.patterns_found.push(PatternState {
                pattern_type: "layering".into(),
                risk_level: "HIGH".into(),
                resolved: false,
            });
            println!("│ ⚠ HIGH: Deep layering detected ({} levels)", chain_depth);
        }

        // Check for opacity jurisdictions
        let opacity_count = self.count_opacity_jurisdictions();
        if opacity_count >= 2 {
            self.state.patterns_found.push(PatternState {
                pattern_type: "opacity_jurisdiction".into(),
                risk_level: "MEDIUM".into(),
                resolved: false,
            });
            println!("│ ⚠ MEDIUM: {} opacity jurisdictions", opacity_count);
        }

        // Check for inconsistencies between claimed and GLEIF
        self.detect_gleif_mismatches().await;

        println!("│ Patterns found: {}", self.state.patterns_found.len());
        println!("│ Inconsistencies: {}", self.state.inconsistencies.len());
        println!("└──────────────────────────────────────────────────────────────┘\n");
    }

    async fn phase_challenge(&mut self) {
        println!("┌─ PHASE 4: CHALLENGE ────────────────────────────────────────┐");

        // Challenge unresolved inconsistencies
        for inconsistency in &self.state.inconsistencies.clone() {
            if !inconsistency.resolved {
                println!("│ Challenging: {}", inconsistency.description);
                
                let response = self.client
                    .respond_to_challenge("inconsistency")
                    .await;

                match response {
                    ChallengeResponse::Truthful { explanation } => {
                        println!("│   → Truthful response: {}", explanation);
                        self.resolve_inconsistency(&inconsistency.description);
                    }
                    ChallengeResponse::Deceptive { false_explanation } => {
                        println!("│   → Deceptive response: {}", false_explanation);
                        // Deception might not be detected immediately
                    }
                    ChallengeResponse::Inconsistent { new_claim } => {
                        println!("│   → Story changed: {}", new_claim);
                        self.add_pattern("changing_story", "HIGH");
                    }
                    ChallengeResponse::Refuses => {
                        println!("│   → Client refused to explain");
                        self.add_pattern("evasion", "HIGH");
                    }
                    ChallengeResponse::Evades => {
                        println!("│   → Client evading");
                        self.add_pattern("evasion", "HIGH");
                    }
                }

                self.log.challenges_issued.push(ChallengeLog {
                    challenge_type: "inconsistency".into(),
                    entity: "".into(),
                    response: format!("{:?}", response),
                });
            }
        }

        // Challenge high-risk patterns
        for pattern in &self.state.patterns_found.clone() {
            if pattern.risk_level == "HIGH" && !pattern.resolved {
                println!("│ Challenging pattern: {}", pattern.pattern_type);
                
                let response = self.client
                    .respond_to_challenge(&pattern.pattern_type)
                    .await;

                match response {
                    ChallengeResponse::Truthful { explanation } => {
                        println!("│   → Explained: {}", explanation);
                        self.resolve_pattern(&pattern.pattern_type);
                    }
                    _ => {
                        println!("│   → Not satisfactorily explained");
                    }
                }
            }
        }

        println!("└──────────────────────────────────────────────────────────────┘\n");
    }

    async fn phase_decision(&mut self) {
        println!("┌─ PHASE 5: DECISION ─────────────────────────────────────────┐");

        // Check for critical blockers
        let has_critical = self.state.patterns_found.iter()
            .any(|p| p.risk_level == "CRITICAL" && !p.resolved);

        let has_unresolved_high = self.state.patterns_found.iter()
            .any(|p| p.risk_level == "HIGH" && !p.resolved);

        let has_unresolved_inconsistencies = self.state.inconsistencies.iter()
            .any(|i| !i.resolved);

        self.recalculate_confidence();

        let decision = if has_critical {
            println!("│ ✗ BLOCKED: Critical pattern unresolved");
            Decision::Blocked
        } else if has_unresolved_high || has_unresolved_inconsistencies {
            println!("│ → ESCALATED: Needs human review");
            println!("│   Unresolved patterns: {}", 
                self.state.patterns_found.iter().filter(|p| !p.resolved).count());
            println!("│   Unresolved inconsistencies: {}",
                self.state.inconsistencies.iter().filter(|i| !i.resolved).count());
            self.log.escalations.push("Unresolved issues require human review".into());
            Decision::Escalated
        } else if self.state.overall_confidence >= 0.80 {
            println!("│ ✓ APPROVED: Confidence {:.0}%", self.state.overall_confidence * 100.0);
            Decision::Approved
        } else if self.state.overall_confidence >= 0.60 {
            println!("│ → ESCALATED: Confidence only {:.0}%", self.state.overall_confidence * 100.0);
            Decision::Escalated
        } else {
            println!("│ ✗ REJECTED: Confidence too low ({:.0}%)", self.state.overall_confidence * 100.0);
            Decision::Rejected
        };

        self.state.decision = Some(decision);

        println!("│");
        println!("│ FINAL DECISION: {:?}", decision);
        println!("└──────────────────────────────────────────────────────────────┘\n");
    }

    fn register_claim(&mut self, claim_type: &str, subject: &str, confidence: f32) {
        self.state.claims.insert(claim_type.to_string(), ClaimState {
            claim_type: claim_type.to_string(),
            subject: subject.to_string(),
            confidence,
            verified: false,
            sources: vec!["client".into()],
        });
        self.log.verbs_executed.push(format!("verify.register-claim:{}", claim_type));
    }

    fn verify_claim(&mut self, claim_key: &str, source: &str, success: bool) {
        if let Some(claim) = self.state.claims.get_mut(claim_key) {
            claim.sources.push(source.to_string());
            if success {
                claim.confidence = (claim.confidence + 0.35).min(0.95);
                claim.verified = true;
            }
        }
        self.log.verbs_executed.push(format!("verify.verify-against-{}", source.to_lowercase()));
    }

    fn add_inconsistency(&mut self, description: &str) {
        self.state.inconsistencies.push(InconsistencyState {
            description: description.to_string(),
            resolved: false,
        });
    }

    fn resolve_inconsistency(&mut self, description: &str) {
        if let Some(inc) = self.state.inconsistencies.iter_mut()
            .find(|i| i.description == description) {
            inc.resolved = true;
        }
    }

    fn add_pattern(&mut self, pattern_type: &str, risk_level: &str) {
        self.state.patterns_found.push(PatternState {
            pattern_type: pattern_type.to_string(),
            risk_level: risk_level.to_string(),
            resolved: false,
        });
        self.log.patterns_detected.push(pattern_type.to_string());
    }

    fn resolve_pattern(&mut self, pattern_type: &str) {
        if let Some(p) = self.state.patterns_found.iter_mut()
            .find(|p| p.pattern_type == pattern_type) {
            p.resolved = true;
        }
    }

    fn recalculate_confidence(&mut self) {
        if self.state.claims.is_empty() {
            self.state.overall_confidence = 0.0;
            return;
        }

        let sum: f32 = self.state.claims.values().map(|c| c.confidence).sum();
        self.state.overall_confidence = sum / self.state.claims.len() as f32;

        // Apply penalties for unresolved issues
        let unresolved_patterns = self.state.patterns_found.iter()
            .filter(|p| !p.resolved)
            .count();
        let unresolved_inconsistencies = self.state.inconsistencies.iter()
            .filter(|i| !i.resolved)
            .count();

        self.state.overall_confidence -= unresolved_patterns as f32 * 0.10;
        self.state.overall_confidence -= unresolved_inconsistencies as f32 * 0.15;
        self.state.overall_confidence = self.state.overall_confidence.max(0.0);
    }

    fn detect_circular_ownership(&self) -> bool {
        // Simplified: check if any owner appears twice in chain
        let mut seen = std::collections::HashSet::new();
        for ownership in &self.scenario.claimed_structure.ownership_chain {
            if !seen.insert(&ownership.owner) {
                return true;
            }
        }
        false
    }

    fn count_opacity_jurisdictions(&self) -> usize {
        let opacity = ["VG", "KY", "PA", "SC", "BZ"];
        let mut count = 0;
        
        // Check CBU
        if opacity.contains(&self.scenario.claimed_structure.cbu.jurisdiction.as_str()) {
            count += 1;
        }
        
        // Check chain (simplified - would need entity jurisdictions)
        count
    }

    async fn detect_gleif_mismatches(&mut self) {
        // Compare claimed structure with GLEIF data
        for ownership in &self.scenario.claimed_structure.ownership_chain {
            if let Some(gleif) = self.gleif.search_name(&ownership.owned).await.first() {
                if let Some(parent) = &gleif.parent_name {
                    if parent.to_lowercase() != ownership.owner.to_lowercase() {
                        self.add_inconsistency(&format!(
                            "GLEIF shows parent '{}' but client claims parent '{}'",
                            parent, ownership.owner
                        ));
                        self.add_pattern("registry_mismatch", "HIGH");
                    }
                }
            }
        }
    }

    fn check_assertions(&self, phase: Phase) {
        for assertion in &self.scenario.assertions {
            if assertion.check_after == phase {
                // Would check assertion here
            }
        }
    }

    fn validate_outcome(&self) -> ScenarioResult {
        let actual_decision = self.state.decision.unwrap_or(Decision::Blocked);
        let expected_decision = self.scenario.expected_outcome.decision;
        
        let decision_match = actual_decision == expected_decision;

        let patterns_match = self.scenario.expected_outcome.patterns_detected.iter()
            .all(|p| self.log.patterns_detected.contains(p));

        let passed = decision_match && patterns_match;

        println!("═══════════════════════════════════════════════════════════════");
        println!("RESULT: {}", if passed { "PASSED ✓" } else { "FAILED ✗" });
        println!("═══════════════════════════════════════════════════════════════");
        println!("Expected decision: {:?}", expected_decision);
        println!("Actual decision:   {:?}", actual_decision);
        println!("Decision match:    {}", if decision_match { "✓" } else { "✗" });
        println!("Patterns match:    {}", if patterns_match { "✓" } else { "✗" });
        println!("═══════════════════════════════════════════════════════════════\n");

        ScenarioResult {
            scenario_id: self.scenario.id.clone(),
            passed,
            actual_decision,
            expected_decision,
            execution_log: self.log.clone(),
            final_state: self.state.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ScenarioResult {
    pub scenario_id: String,
    pub passed: bool,
    pub actual_decision: Decision,
    pub expected_decision: Decision,
    pub execution_log: ExecutionLog,
    pub final_state: VerificationState,
}
```

### 3.2 Tasks - Test Runner

- [ ] Create `rust/tests/harness/runner.rs`
- [ ] Implement `ScenarioRunner`
- [ ] Implement all phases
- [ ] Implement pattern detection
- [ ] Implement challenge handling
- [ ] Implement decision logic
- [ ] Implement assertion checking

---

## Part 4: Pre-built Test Scenarios

### 4.1 Honest Client Scenarios

**File:** `rust/tests/scenarios/honest.rs`

```rust
//! Honest Client Test Scenarios

use crate::harness::scenario::*;

/// Simple honest client - straightforward structure, all docs available
pub fn simple_honest() -> TestScenario {
    TestScenario {
        id: "honest_simple".into(),
        name: "Simple Honest Client".into(),
        description: "Straightforward fund structure with cooperative client".into(),
        client_type: ClientType::Honest,
        
        claimed_structure: ClaimedStructure {
            cbu: EntityClaim {
                name: "Acme Growth Fund".into(),
                entity_type: "FUND".into(),
                jurisdiction: "LU".into(),
                lei: Some("5493001KJTIIGC8Y1R12".into()),
                registration_number: Some("B123456".into()),
            },
            ownership_chain: vec![
                OwnershipClaim {
                    owner: "Acme Asset Management".into(),
                    owned: "Acme Growth Fund".into(),
                    percentage: 100.0,
                    source_document: Some("prospectus".into()),
                },
                OwnershipClaim {
                    owner: "Acme Holdings Ltd".into(),
                    owned: "Acme Asset Management".into(),
                    percentage: 100.0,
                    source_document: Some("share_register".into()),
                },
                OwnershipClaim {
                    owner: "John Smith".into(),
                    owned: "Acme Holdings Ltd".into(),
                    percentage: 100.0,
                    source_document: Some("share_register".into()),
                },
            ],
            control_persons: vec![
                ControlPersonClaim {
                    person_name: "John Smith".into(),
                    role: "Director".into(),
                    entity: "Acme Holdings Ltd".into(),
                },
            ],
            documents: vec![
                DocumentClaim {
                    document_type: "prospectus".into(),
                    entity: "Acme Growth Fund".into(),
                    is_authentic: true,
                    is_current: true,
                    content_matches_claim: true,
                },
            ],
        },
        
        ground_truth: GroundTruth {
            true_ownership: vec![], // Same as claimed
            true_control: vec![],
            hidden_entities: vec![],
            true_ubo: Some("John Smith".into()),
        },
        
        external_sources: ExternalSources {
            gleif_data: HashMap::from([
                ("5493001KJTIIGC8Y1R12".into(), GleifRecord {
                    lei: "5493001KJTIIGC8Y1R12".into(),
                    legal_name: "Acme Growth Fund".into(),
                    jurisdiction: "LU".into(),
                    parent_lei: Some("5493001KJTIIGC8Y1R13".into()),
                    parent_name: Some("Acme Asset Management".into()),
                    ultimate_parent_lei: Some("5493001KJTIIGC8Y1R14".into()),
                    entity_status: "ACTIVE".into(),
                }),
            ]),
            registry_data: HashMap::new(),
            screening_results: HashMap::from([
                ("John Smith".into(), ScreeningResult {
                    entity_name: "John Smith".into(),
                    pep_hit: false,
                    pep_details: None,
                    sanctions_hit: false,
                    sanctions_details: None,
                    adverse_media_hit: false,
                    adverse_media_details: None,
                }),
            ]),
        },
        
        client_behavior: ClientBehavior {
            response_delay: ResponseDelay::Prompt,
            challenge_responses: HashMap::from([
                ("inconsistency".into(), ChallengeResponse::Truthful {
                    explanation: "Happy to clarify any questions".into(),
                }),
            ]),
            refused_documents: vec![],
        },
        
        expected_outcome: ExpectedOutcome {
            decision: Decision::Approved,
            min_confidence: Some(0.80),
            patterns_detected: vec![],
            escalated: false,
            reason: "Simple structure, fully verified, no issues".into(),
        },
        
        assertions: vec![],
    }
}

/// Honest client with listed company exemption
pub fn honest_listed_parent() -> TestScenario {
    TestScenario {
        id: "honest_listed".into(),
        name: "Honest Client - Listed Parent".into(),
        description: "Fund owned by publicly listed company".into(),
        client_type: ClientType::Honest,
        // ... structure with Allianz-like pattern
        expected_outcome: ExpectedOutcome {
            decision: Decision::Approved,
            min_confidence: Some(0.85),
            patterns_detected: vec![],
            escalated: false,
            reason: "Listed company exemption applies".into(),
        },
        ..Default::default()
    }
}
```

### 4.2 Adversarial Scenarios

**File:** `rust/tests/scenarios/adversarial.rs`

```rust
//! Adversarial Client Test Scenarios
//!
//! These MUST be caught by the verification system.

use crate::harness::scenario::*;

/// Circular ownership - A owns B owns C owns A
pub fn circular_ownership() -> TestScenario {
    TestScenario {
        id: "adversarial_circular".into(),
        name: "Circular Ownership Attempt".into(),
        description: "Client hides UBO via circular ownership structure".into(),
        client_type: ClientType::Adversarial,
        
        claimed_structure: ClaimedStructure {
            cbu: EntityClaim {
                name: "Shadow Fund".into(),
                entity_type: "FUND".into(),
                jurisdiction: "KY".into(),
                lei: None,
                registration_number: None,
            },
            ownership_chain: vec![
                OwnershipClaim {
                    owner: "Alpha Holdings".into(),
                    owned: "Shadow Fund".into(),
                    percentage: 100.0,
                    source_document: Some("client_letter".into()),
                },
                OwnershipClaim {
                    owner: "Beta Corp".into(),
                    owned: "Alpha Holdings".into(),
                    percentage: 100.0,
                    source_document: Some("client_letter".into()),
                },
                OwnershipClaim {
                    owner: "Gamma Ltd".into(),
                    owned: "Beta Corp".into(),
                    percentage: 100.0,
                    source_document: Some("client_letter".into()),
                },
                OwnershipClaim {
                    owner: "Alpha Holdings".into(),  // CIRCULAR!
                    owned: "Gamma Ltd".into(),
                    percentage: 100.0,
                    source_document: Some("client_letter".into()),
                },
            ],
            control_persons: vec![],
            documents: vec![],
        },
        
        ground_truth: GroundTruth {
            true_ownership: vec![],
            true_control: vec![],
            hidden_entities: vec![],
            true_ubo: Some("Hidden Person X".into()),  // The real UBO
        },
        
        external_sources: ExternalSources {
            gleif_data: HashMap::new(),  // No LEIs - suspicious
            registry_data: HashMap::new(),
            screening_results: HashMap::new(),
        },
        
        client_behavior: ClientBehavior {
            response_delay: ResponseDelay::Slow,
            challenge_responses: HashMap::from([
                ("circular_ownership".into(), ChallengeResponse::Evades),
                ("inconsistency".into(), ChallengeResponse::Inconsistent {
                    new_claim: "Actually it's a different structure...".into(),
                }),
            ]),
            refused_documents: vec!["share_register".into()],
        },
        
        expected_outcome: ExpectedOutcome {
            decision: Decision::Blocked,
            min_confidence: None,
            patterns_detected: vec!["circular_ownership".into()],
            escalated: false,
            reason: "Circular ownership is critical pattern - must block".into(),
        },
        
        assertions: vec![
            Assertion {
                check_after: Phase::PatternDetection,
                assertion_type: AssertionType::PatternDetected {
                    pattern_type: "circular_ownership".into(),
                },
                expected: "true".into(),
            },
        ],
    }
}

/// Registry mismatch - GLEIF says one thing, client says another
pub fn registry_mismatch() -> TestScenario {
    TestScenario {
        id: "adversarial_mismatch".into(),
        name: "Registry Mismatch - False Parent".into(),
        description: "Client claims different parent than registry shows".into(),
        client_type: ClientType::Adversarial,
        
        claimed_structure: ClaimedStructure {
            cbu: EntityClaim {
                name: "Legitimate Fund".into(),
                entity_type: "FUND".into(),
                jurisdiction: "LU".into(),
                lei: Some("549300TESTLEI00001".into()),
                registration_number: Some("B999999".into()),
            },
            ownership_chain: vec![
                OwnershipClaim {
                    owner: "Fake Parent Corp".into(),  // FALSE
                    owned: "Legitimate Fund".into(),
                    percentage: 100.0,
                    source_document: Some("forged_doc".into()),
                },
            ],
            control_persons: vec![],
            documents: vec![],
        },
        
        ground_truth: GroundTruth {
            true_ownership: vec![
                OwnershipClaim {
                    owner: "Real Parent Corp".into(),  // TRUE
                    owned: "Legitimate Fund".into(),
                    percentage: 100.0,
                    source_document: None,
                },
            ],
            true_control: vec![],
            hidden_entities: vec![],
            true_ubo: Some("Criminal Person".into()),
        },
        
        external_sources: ExternalSources {
            gleif_data: HashMap::from([
                ("549300TESTLEI00001".into(), GleifRecord {
                    lei: "549300TESTLEI00001".into(),
                    legal_name: "Legitimate Fund".into(),
                    jurisdiction: "LU".into(),
                    parent_lei: Some("549300REALPARENT01".into()),
                    parent_name: Some("Real Parent Corp".into()),  // TRUTH
                    ultimate_parent_lei: None,
                    entity_status: "ACTIVE".into(),
                }),
            ]),
            registry_data: HashMap::new(),
            screening_results: HashMap::new(),
        },
        
        client_behavior: ClientBehavior {
            response_delay: ResponseDelay::Selective {
                fast_types: vec!["prospectus".into()],
                slow_types: vec!["share_register".into()],
            },
            challenge_responses: HashMap::from([
                ("registry_mismatch".into(), ChallengeResponse::Deceptive {
                    false_explanation: "GLEIF is outdated, we changed ownership recently".into(),
                }),
            ]),
            refused_documents: vec![],
        },
        
        expected_outcome: ExpectedOutcome {
            decision: Decision::Escalated,
            min_confidence: None,
            patterns_detected: vec!["registry_mismatch".into()],
            escalated: true,
            reason: "Registry mismatch cannot be verified - needs human review".into(),
        },
        
        assertions: vec![
            Assertion {
                check_after: Phase::Verification,
                assertion_type: AssertionType::InconsistencyFound {
                    description: "parent mismatch".into(),
                },
                expected: "true".into(),
            },
        ],
    }
}

/// Hidden nominee structure
pub fn hidden_nominee() -> TestScenario {
    TestScenario {
        id: "adversarial_nominee".into(),
        name: "Hidden Nominee Structure".into(),
        description: "Client uses nominees to hide true UBO".into(),
        client_type: ClientType::Adversarial,
        // ... nominee pattern
        expected_outcome: ExpectedOutcome {
            decision: Decision::Escalated,
            min_confidence: None,
            patterns_detected: vec!["nominee_usage".into()],
            escalated: true,
            reason: "Nominee pattern requires investigation".into(),
        },
        ..Default::default()
    }
}

/// Deep layering through secrecy jurisdictions
pub fn opacity_layering() -> TestScenario {
    TestScenario {
        id: "adversarial_opacity".into(),
        name: "Opacity Jurisdiction Layering".into(),
        description: "Multiple layers through BVI, Cayman, Panama".into(),
        client_type: ClientType::Adversarial,
        // ... 6+ layer structure through opacity jurisdictions
        expected_outcome: ExpectedOutcome {
            decision: Decision::Escalated,
            min_confidence: None,
            patterns_detected: vec!["layering".into(), "opacity_jurisdiction".into()],
            escalated: true,
            reason: "Complex structure in secrecy jurisdictions".into(),
        },
        ..Default::default()
    }
}

/// Evasive behavior - delays, refuses documents
pub fn evasive_client() -> TestScenario {
    TestScenario {
        id: "adversarial_evasive".into(),
        name: "Evasive Client Behavior".into(),
        description: "Client delays, refuses key documents, changes story".into(),
        client_type: ClientType::Adversarial,
        // ... normal structure but evasive behavior
        client_behavior: ClientBehavior {
            response_delay: ResponseDelay::Selective {
                fast_types: vec!["prospectus".into()],
                slow_types: vec!["share_register".into(), "source_of_funds".into()],
            },
            challenge_responses: HashMap::from([
                ("inconsistency".into(), ChallengeResponse::Evades),
            ]),
            refused_documents: vec!["source_of_funds".into()],
        },
        expected_outcome: ExpectedOutcome {
            decision: Decision::Escalated,
            min_confidence: None,
            patterns_detected: vec!["evasion".into()],
            escalated: true,
            reason: "Evasive behavior pattern detected".into(),
        },
        ..Default::default()
    }
}
```

### 4.3 Tasks - Test Scenarios

- [ ] Create `rust/tests/scenarios/` directory
- [ ] Create `rust/tests/scenarios/honest.rs`
- [ ] Implement `simple_honest` scenario
- [ ] Implement `honest_listed_parent` scenario
- [ ] Create `rust/tests/scenarios/adversarial.rs`
- [ ] Implement `circular_ownership` scenario
- [ ] Implement `registry_mismatch` scenario
- [ ] Implement `hidden_nominee` scenario
- [ ] Implement `opacity_layering` scenario
- [ ] Implement `evasive_client` scenario

---

## Part 5: Test Suite Runner

### 5.1 Suite Definition

**File:** `rust/tests/harness/suite.rs`

```rust
//! Test Suite Runner
//!
//! Runs all scenarios and produces report.

use super::runner::*;
use super::scenario::*;

pub struct TestSuite {
    scenarios: Vec<TestScenario>,
}

impl TestSuite {
    pub fn new() -> Self {
        Self { scenarios: vec![] }
    }

    pub fn add(&mut self, scenario: TestScenario) {
        self.scenarios.push(scenario);
    }

    pub fn add_all_honest(&mut self) {
        self.add(super::scenarios::honest::simple_honest());
        self.add(super::scenarios::honest::honest_listed_parent());
    }

    pub fn add_all_adversarial(&mut self) {
        self.add(super::scenarios::adversarial::circular_ownership());
        self.add(super::scenarios::adversarial::registry_mismatch());
        self.add(super::scenarios::adversarial::hidden_nominee());
        self.add(super::scenarios::adversarial::opacity_layering());
        self.add(super::scenarios::adversarial::evasive_client());
    }

    pub async fn run_all(&self) -> SuiteResult {
        let mut results = Vec::new();
        
        println!("\n");
        println!("╔═══════════════════════════════════════════════════════════════╗");
        println!("║          UBO VERIFICATION TEST SUITE                          ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        for scenario in &self.scenarios {
            let mut runner = ScenarioRunner::new(scenario.clone());
            let result = runner.run().await;
            results.push(result);
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let failed = results.iter().filter(|r| !r.passed).count();

        println!("\n");
        println!("╔═══════════════════════════════════════════════════════════════╗");
        println!("║          SUITE SUMMARY                                        ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║  Total scenarios: {:3}                                        ║", results.len());
        println!("║  Passed:          {:3}  ✓                                     ║", passed);
        println!("║  Failed:          {:3}  ✗                                     ║", failed);
        println!("╚═══════════════════════════════════════════════════════════════╝\n");

        // Detailed failures
        if failed > 0 {
            println!("FAILURES:");
            for result in results.iter().filter(|r| !r.passed) {
                println!("  • {} - Expected {:?}, got {:?}",
                    result.scenario_id,
                    result.expected_decision,
                    result.actual_decision
                );
            }
            println!();
        }

        SuiteResult {
            total: results.len(),
            passed,
            failed,
            results,
        }
    }
}

#[derive(Debug)]
pub struct SuiteResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<ScenarioResult>,
}

impl SuiteResult {
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

/// Main entry point for running tests
pub async fn run_verification_tests() -> SuiteResult {
    let mut suite = TestSuite::new();
    
    // Add all scenarios
    suite.add_all_honest();
    suite.add_all_adversarial();
    
    suite.run_all().await
}
```

### 5.2 CLI Runner

**File:** `rust/tests/harness/main.rs`

```rust
//! Test Harness CLI
//!
//! Run with: cargo test --test verification_harness

mod harness;
mod scenarios;

use harness::suite::run_verification_tests;

#[tokio::main]
async fn main() {
    let result = run_verification_tests().await;
    
    if result.all_passed() {
        println!("All verification tests passed! ✓");
        std::process::exit(0);
    } else {
        println!("Some verification tests failed! ✗");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_honest_client_approved() {
        let scenario = scenarios::honest::simple_honest();
        let mut runner = harness::runner::ScenarioRunner::new(scenario);
        let result = runner.run().await;
        assert!(result.passed, "Honest client should be approved");
    }

    #[tokio::test]
    async fn test_circular_ownership_blocked() {
        let scenario = scenarios::adversarial::circular_ownership();
        let mut runner = harness::runner::ScenarioRunner::new(scenario);
        let result = runner.run().await;
        assert!(result.passed, "Circular ownership should be blocked");
        assert!(
            result.execution_log.patterns_detected.contains(&"circular_ownership".to_string()),
            "Should detect circular ownership pattern"
        );
    }

    #[tokio::test]
    async fn test_registry_mismatch_escalated() {
        let scenario = scenarios::adversarial::registry_mismatch();
        let mut runner = harness::runner::ScenarioRunner::new(scenario);
        let result = runner.run().await;
        assert!(result.passed, "Registry mismatch should be escalated");
    }

    #[tokio::test]
    async fn test_full_suite() {
        let result = run_verification_tests().await;
        assert!(result.all_passed(), "All scenarios should pass");
    }
}
```

### 5.3 Tasks - Test Suite

- [ ] Create `rust/tests/harness/suite.rs`
- [ ] Implement `TestSuite`
- [ ] Implement summary reporting
- [ ] Create `rust/tests/harness/main.rs`
- [ ] Create individual test functions
- [ ] Create `cargo test` integration

---

## Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TEST HARNESS STRUCTURE                                                     │
│                                                                             │
│  rust/tests/                                                               │
│  ├── harness/                                                              │
│  │   ├── mod.rs                                                            │
│  │   ├── scenario.rs          # Scenario definition types                  │
│  │   ├── mock_sources.rs      # Mock GLEIF, Registry, Screening            │
│  │   ├── runner.rs            # Executes scenarios                         │
│  │   └── suite.rs             # Runs all tests                             │
│  ├── scenarios/                                                            │
│  │   ├── mod.rs                                                            │
│  │   ├── honest.rs            # Honest client scenarios                    │
│  │   └── adversarial.rs       # Adversarial scenarios                      │
│  └── main.rs                  # CLI entry point                            │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  SCENARIOS                                                                  │
│                                                                             │
│  HONEST (should pass quickly):                                             │
│  • simple_honest           Simple structure, all docs, prompt client       │
│  • honest_listed_parent    Listed company exemption applies                │
│                                                                             │
│  ADVERSARIAL (must be caught):                                             │
│  • circular_ownership      A→B→C→A - must BLOCK                            │
│  • registry_mismatch       GLEIF contradicts client - must ESCALATE        │
│  • hidden_nominee          Nominee structure - must ESCALATE               │
│  • opacity_layering        BVI→Cayman→Panama - must ESCALATE               │
│  • evasive_client          Delays, refuses docs - must ESCALATE            │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  SUCCESS CRITERIA                                                           │
│                                                                             │
│  ✓ Honest clients verified efficiently (< 5 steps)                         │
│  ✓ Circular ownership ALWAYS blocked                                       │
│  ✓ Registry mismatches ALWAYS detected                                     │
│  ✓ Nominee patterns detected                                               │
│  ✓ Evasion behavior detected                                               │
│  ✓ No false approvals of adversarial cases                                 │
│                                                                             │
│  THE STANDARD: Would this catch a sophisticated liar?                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Example Output

```
╔═══════════════════════════════════════════════════════════════╗
║          UBO VERIFICATION TEST SUITE                          ║
╚═══════════════════════════════════════════════════════════════╝

═══════════════════════════════════════════════════════════════
SCENARIO: Simple Honest Client
TYPE: Honest
═══════════════════════════════════════════════════════════════

┌─ PHASE 1: REGISTRATION ─────────────────────────────────────┐
│ Registered 4 claims
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 2: VERIFICATION ─────────────────────────────────────┐
│ ✓ GLEIF verified: Acme Growth Fund
│ ✓ Registry verified: Acme Growth Fund
│ Overall confidence: 85%
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 3: PATTERN DETECTION ────────────────────────────────┐
│ Patterns found: 0
│ Inconsistencies: 0
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 4: CHALLENGE ────────────────────────────────────────┐
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 5: DECISION ─────────────────────────────────────────┐
│ ✓ APPROVED: Confidence 85%
│
│ FINAL DECISION: Approved
└──────────────────────────────────────────────────────────────┘

═══════════════════════════════════════════════════════════════
RESULT: PASSED ✓
═══════════════════════════════════════════════════════════════

═══════════════════════════════════════════════════════════════
SCENARIO: Circular Ownership Attempt
TYPE: Adversarial
═══════════════════════════════════════════════════════════════

┌─ PHASE 1: REGISTRATION ─────────────────────────────────────┐
│ Registered 5 claims
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 2: VERIFICATION ─────────────────────────────────────┐
│ ✗ GLEIF not found: Alpha Holdings
│ ✗ GLEIF not found: Beta Corp
│ Overall confidence: 35%
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 3: PATTERN DETECTION ────────────────────────────────┐
│ ⚠ CRITICAL: Circular ownership detected!
│ Patterns found: 1
│ Inconsistencies: 0
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 4: CHALLENGE ────────────────────────────────────────┐
│ Challenging pattern: circular_ownership
│   → Client evading
└──────────────────────────────────────────────────────────────┘

┌─ PHASE 5: DECISION ─────────────────────────────────────────┐
│ ✗ BLOCKED: Critical pattern unresolved
│
│ FINAL DECISION: Blocked
└──────────────────────────────────────────────────────────────┘

═══════════════════════════════════════════════════════════════
RESULT: PASSED ✓
═══════════════════════════════════════════════════════════════

╔═══════════════════════════════════════════════════════════════╗
║          SUITE SUMMARY                                        ║
╠═══════════════════════════════════════════════════════════════╣
║  Total scenarios:   7                                        ║
║  Passed:            7  ✓                                     ║
║  Failed:            0  ✗                                     ║
╚═══════════════════════════════════════════════════════════════╝
```

---

*Animate the game. Prove the agent catches liars.*
