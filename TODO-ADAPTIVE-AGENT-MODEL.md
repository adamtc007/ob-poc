# TODO: Adaptive Agent Model (Intent → End State → Gaps → Verbs)

## ⛔ MANDATORY FIRST STEP

**Read these files before starting:**
- `/EGUI-RULES.md` - UI patterns and constraints
- `/rust/config/verbs/` - All existing verb definitions
- `/rust/src/dsl_v2/runtime_registry.rs` - Verb execution model
- `/TODO-GRAPH-DSL-DOMAIN.md` - Graph query verbs (dependency)
- `/ALLIANZ-DATA-ACQUISITION.md` - Context on UBO data model

---

## Overview

This is the **agentic orchestration layer**. The agent doesn't follow workflows - 
it closes gaps between current state and a defined end state.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  THE MODEL                                                                  │
│                                                                             │
│  USER INTENT          "Make CBU @fund KYC ready"                           │
│       │                                                                     │
│       ▼                                                                     │
│  END STATE MODEL      What does "KYC ready" mean? (declarative)            │
│       │                                                                     │
│       ▼                                                                     │
│  GAP ANALYSIS         Current state vs End state = Gaps                    │
│       │                                                                     │
│       ▼                                                                     │
│  VERB SELECTION       Which verbs close which gaps?                        │
│       │                                                                     │
│       ▼                                                                     │
│  EXECUTION            Agent executes verbs via DSL engine                  │
│       │                                                                     │
│       ▼                                                                     │
│  RE-EVALUATION        New state → New gaps → Continue until done           │
│       │                                                                     │
│       ▼                                                                     │
│  FEEDBACK CAPTURE     Execution data → Model learning                      │
│                                                                             │
│  Agent moves the chess pieces (verbs) on the CBU/UBO board.               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: End State Model

### 1.1 Core Concepts

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  END STATE = Collection of REQUIREMENTS that must ALL be satisfied         │
│                                                                             │
│  REQUIREMENT = A condition that can be:                                    │
│    • Evaluated (true/false/partial)                                        │
│    • Explained (why not met)                                               │
│    • Resolved (which verbs can fix it)                                     │
│                                                                             │
│  GAP = Requirement that is NOT satisfied                                   │
│    • Has a type (what's missing)                                           │
│    • Has context (why it matters)                                          │
│    • Has resolution paths (how to fix)                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 End State Definition Schema

**File:** `rust/config/end_states/schema.yaml`

```yaml
# End State Definition Schema
# 
# An end state is a goal the agent works toward.
# It's composed of requirements that can be evaluated against current state.

end_state:
  # Unique identifier
  id: string
  
  # Human readable name
  name: string
  
  # What entity type this applies to
  entity_type: string  # cbu, entity, kyc_case, etc.
  
  # Description for agent context
  description: string
  
  # Requirements that must be met
  requirements:
    - id: string
      name: string
      description: string
      
      # How to evaluate this requirement
      evaluation:
        # Query to run (returns data for evaluation)
        query: dsl_expression
        # Condition that must be true
        condition: expression
        # What to check
        type: enum [exists, count, status, all_of, any_of, custom]
      
      # What gaps mean when not met
      gap:
        type: string
        severity: enum [blocking, warning, info]
        message_template: string
      
      # How to resolve this gap
      resolution:
        # Verbs that can address this gap
        verbs: [verb_pattern]
        # Prerequisites (other requirements that must be met first)
        requires: [requirement_id]
        # Can agent auto-resolve or needs human?
        auto_resolve: boolean
        # Hints for agent
        strategy: string

  # Sub-requirements that depend on entity type, jurisdiction, etc.
  conditional_requirements:
    - condition: expression
      requirements: [requirement]
```

### 1.3 KYC Ready End State

**File:** `rust/config/end_states/kyc_ready.yaml`

```yaml
end_state:
  id: kyc_ready
  name: "KYC Ready"
  entity_type: cbu
  description: |
    CBU has completed all KYC requirements and is ready for 
    compliance decision. UBO chain verified, documents collected,
    screening complete, risk assessed.

  requirements:
    # =========================================================================
    # ENTITY FOUNDATION
    # =========================================================================
    - id: entity_exists
      name: "Entity Exists"
      description: "CBU entity record exists with basic data"
      evaluation:
        type: exists
        query: (entity.get :id $cbu_id)
        condition: (not-nil? result)
      gap:
        type: missing_entity
        severity: blocking
        message_template: "CBU entity does not exist"
      resolution:
        verbs: [entity.create-*]
        auto_resolve: false
        strategy: "Entity must be created before any other work"

    - id: jurisdiction_set
      name: "Jurisdiction Set"
      description: "CBU has jurisdiction assigned"
      evaluation:
        type: exists
        query: (entity.get :id $cbu_id :field jurisdiction)
        condition: (not-blank? result)
      gap:
        type: missing_jurisdiction
        severity: blocking
        message_template: "Jurisdiction not set for {cbu_name}"
      resolution:
        verbs: [entity.update]
        auto_resolve: true
        strategy: "Infer from ManCo location or fund domicile"

    # =========================================================================
    # UBO CHAIN - OWNERSHIP PRONG
    # =========================================================================
    - id: ubo_chain_exists
      name: "UBO Chain Started"
      description: "At least one ownership relationship exists"
      evaluation:
        type: count
        query: (ubo.list-ownership :entity $cbu_id)
        condition: (> count 0)
      gap:
        type: no_ubo_chain
        severity: blocking
        message_template: "No ownership chain defined for {cbu_name}"
      resolution:
        verbs: [ubo.add-ownership]
        auto_resolve: true
        strategy: "Search GLEIF for parent entities, add ownership links"

    - id: ubo_chain_complete
      name: "UBO Chain Complete"
      description: "Ownership chain reaches natural persons or exemption"
      evaluation:
        type: custom
        query: (ubo.evaluate-chain :entity $cbu_id)
        condition: |
          (or (= chain_status "COMPLETE")
              (= chain_status "EXEMPTION_APPLIED"))
      gap:
        type: incomplete_ubo_chain
        severity: blocking
        message_template: |
          UBO chain incomplete. {missing_count} entities need 
          parent identification: {missing_entities}
      resolution:
        verbs: [ubo.add-ownership, ubo.apply-exemption]
        requires: [ubo_chain_exists]
        auto_resolve: true
        strategy: |
          For each entity without parent:
          1. Check if exemption applies (listed company, regulated fund)
          2. Search GLEIF for parent
          3. If natural person threshold reached, mark complete
          4. If can't resolve, flag for human review

    - id: ubo_chain_verified
      name: "UBO Chain Verified"
      description: "All ownership relationships have evidence"
      evaluation:
        type: all_of
        query: (ubo.list-ownership :entity $cbu_id :include-evidence true)
        condition: (all? ownership (has-evidence? ownership))
      gap:
        type: unverified_ownership
        severity: blocking
        message_template: |
          {unverified_count} ownership relationships need evidence:
          {unverified_list}
      resolution:
        verbs: [kyc.request-document, kyc.link-evidence]
        requires: [ubo_chain_complete]
        auto_resolve: true
        strategy: |
          For each unverified ownership:
          - Company: request share register, articles of association
          - Trust: request trust deed
          - Fund: request prospectus showing structure

    # =========================================================================
    # UBO CHAIN - CONTROL PRONG
    # =========================================================================
    - id: control_persons_identified
      name: "Control Persons Identified"
      description: "Directors, officers, signatories identified"
      evaluation:
        type: custom
        query: (ubo.list-control-persons :entity $cbu_id)
        condition: |
          (and (>= (count directors) 1)
               (has-senior-management? result))
      gap:
        type: missing_control_persons
        severity: blocking
        message_template: |
          Control persons not fully identified for {entity_name}.
          Need: directors, senior management.
      resolution:
        verbs: [ubo.add-control, kyc.request-document]
        auto_resolve: true
        strategy: |
          1. Request register of directors from entity
          2. Extract names from document
          3. Create person entities
          4. Link as control persons with roles

    # =========================================================================
    # NATURAL PERSON VERIFICATION
    # =========================================================================
    - id: ubo_persons_created
      name: "UBO Persons Created"
      description: "All natural person UBOs have entity records"
      evaluation:
        type: all_of
        query: (ubo.list-natural-persons :cbu $cbu_id)
        condition: (all? person (entity-exists? person))
      gap:
        type: missing_person_entity
        severity: blocking
        message_template: |
          {count} natural persons need entity records created
      resolution:
        verbs: [entity.create-proper-person]
        requires: [ubo_chain_complete, control_persons_identified]
        auto_resolve: true
        strategy: "Create entity for each identified natural person"

    - id: ubo_persons_verified
      name: "UBO Persons Verified"
      description: "All natural person UBOs have ID verification"
      evaluation:
        type: all_of
        query: (ubo.list-natural-persons :cbu $cbu_id :include-verification true)
        condition: (all? person (verified? person))
      gap:
        type: unverified_persons
        severity: blocking
        message_template: |
          {count} persons need ID verification: {person_list}
      resolution:
        verbs: [kyc.request-document]
        requires: [ubo_persons_created]
        auto_resolve: true
        strategy: |
          For each unverified person:
          - Request passport or national ID
          - Request proof of address

    # =========================================================================
    # SCREENING
    # =========================================================================
    - id: pep_screening_complete
      name: "PEP Screening Complete"
      description: "All natural persons screened for PEP status"
      evaluation:
        type: all_of
        query: (screening.list :cbu $cbu_id :type PEP)
        condition: |
          (all? person 
            (and (screening-exists? person "PEP")
                 (screening-current? person "PEP")))
      gap:
        type: missing_pep_screening
        severity: blocking
        message_template: "{count} persons need PEP screening"
      resolution:
        verbs: [screening.pep]
        requires: [ubo_persons_created]
        auto_resolve: true
        strategy: "Run PEP screening for each natural person"

    - id: sanctions_screening_complete
      name: "Sanctions Screening Complete"
      description: "All parties screened against sanctions lists"
      evaluation:
        type: all_of
        query: (screening.list :cbu $cbu_id :type SANCTIONS)
        condition: |
          (all? party 
            (and (screening-exists? party "SANCTIONS")
                 (screening-current? party "SANCTIONS")))
      gap:
        type: missing_sanctions_screening
        severity: blocking
        message_template: "{count} parties need sanctions screening"
      resolution:
        verbs: [screening.sanctions]
        requires: [ubo_persons_created]
        auto_resolve: true
        strategy: "Run sanctions screening for all parties"

    - id: adverse_media_complete
      name: "Adverse Media Complete"
      description: "All parties screened for adverse media"
      evaluation:
        type: all_of
        query: (screening.list :cbu $cbu_id :type ADVERSE_MEDIA)
        condition: |
          (all? party 
            (and (screening-exists? party "ADVERSE_MEDIA")
                 (screening-current? party "ADVERSE_MEDIA")))
      gap:
        type: missing_adverse_media
        severity: warning
        message_template: "{count} parties need adverse media screening"
      resolution:
        verbs: [screening.adverse-media]
        requires: [ubo_persons_created]
        auto_resolve: true
        strategy: "Run adverse media screening for all parties"

    - id: screening_hits_resolved
      name: "Screening Hits Resolved"
      description: "All screening hits reviewed and dispositioned"
      evaluation:
        type: all_of
        query: (screening.list-hits :cbu $cbu_id :status OPEN)
        condition: (= count 0)
      gap:
        type: unresolved_screening_hits
        severity: blocking
        message_template: |
          {count} screening hits need review: {hit_summary}
      resolution:
        verbs: [screening.resolve-hit]
        requires: [pep_screening_complete, sanctions_screening_complete]
        auto_resolve: false
        strategy: "Human review required for screening hits"

    # =========================================================================
    # DOCUMENTS
    # =========================================================================
    - id: required_docs_identified
      name: "Required Documents Identified"
      description: "Document checklist generated based on entity type"
      evaluation:
        type: exists
        query: (kyc.get-doc-checklist :cbu $cbu_id)
        condition: (not-empty? result)
      gap:
        type: no_doc_checklist
        severity: blocking
        message_template: "Document checklist not generated"
      resolution:
        verbs: [kyc.generate-checklist]
        auto_resolve: true
        strategy: |
          Generate checklist based on:
          - Entity type (fund, company, trust)
          - Jurisdiction (LU, IE, DE, etc.)
          - Risk rating
          - Regulatory requirements

    - id: required_docs_requested
      name: "Required Documents Requested"
      description: "All required documents have been requested"
      evaluation:
        type: all_of
        query: (kyc.get-doc-checklist :cbu $cbu_id :include-status true)
        condition: |
          (all? doc 
            (or (doc-collected? doc)
                (doc-requested? doc)))
      gap:
        type: docs_not_requested
        severity: warning
        message_template: "{count} documents need to be requested"
      resolution:
        verbs: [kyc.request-document]
        requires: [required_docs_identified]
        auto_resolve: true
        strategy: "Request each missing document from appropriate party"

    - id: required_docs_collected
      name: "Required Documents Collected"
      description: "All required documents have been received"
      evaluation:
        type: all_of
        query: (kyc.get-doc-checklist :cbu $cbu_id :include-status true)
        condition: (all? doc (doc-collected? doc))
      gap:
        type: docs_outstanding
        severity: blocking
        message_template: |
          {count} documents outstanding: {doc_list}
      resolution:
        verbs: [kyc.request-document, kyc.chase-document]
        requires: [required_docs_requested]
        auto_resolve: false
        strategy: "Follow up on outstanding document requests"

    - id: docs_validated
      name: "Documents Validated"
      description: "All collected documents pass validation"
      evaluation:
        type: all_of
        query: (kyc.list-documents :cbu $cbu_id :include-validation true)
        condition: (all? doc (doc-valid? doc))
      gap:
        type: invalid_documents
        severity: blocking
        message_template: |
          {count} documents have validation issues: {issues}
      resolution:
        verbs: [kyc.reject-document, kyc.request-document]
        requires: [required_docs_collected]
        auto_resolve: false
        strategy: "Review validation failures, request replacement docs"

    # =========================================================================
    # RISK ASSESSMENT
    # =========================================================================
    - id: risk_factors_assessed
      name: "Risk Factors Assessed"
      description: "All risk factors evaluated"
      evaluation:
        type: exists
        query: (risk.get-assessment :cbu $cbu_id)
        condition: (assessment-complete? result)
      gap:
        type: risk_not_assessed
        severity: blocking
        message_template: "Risk assessment not complete"
      resolution:
        verbs: [risk.assess]
        requires: 
          - ubo_chain_complete
          - screening_hits_resolved
        auto_resolve: true
        strategy: |
          Evaluate:
          - Jurisdiction risk
          - Entity type risk
          - UBO structure complexity
          - PEP exposure
          - Industry risk

    - id: risk_rating_assigned
      name: "Risk Rating Assigned"
      description: "Final risk rating calculated and assigned"
      evaluation:
        type: exists
        query: (risk.get-rating :cbu $cbu_id)
        condition: (not-nil? result)
      gap:
        type: no_risk_rating
        severity: blocking
        message_template: "Risk rating not assigned"
      resolution:
        verbs: [risk.assign-rating]
        requires: [risk_factors_assessed]
        auto_resolve: true
        strategy: "Calculate rating from assessed factors"

    # =========================================================================
    # KYC CASE
    # =========================================================================
    - id: kyc_case_exists
      name: "KYC Case Exists"
      description: "KYC case record created"
      evaluation:
        type: exists
        query: (kyc.get-case :cbu $cbu_id)
        condition: (not-nil? result)
      gap:
        type: no_kyc_case
        severity: blocking
        message_template: "KYC case not created"
      resolution:
        verbs: [kyc.create-case]
        auto_resolve: true
        strategy: "Create KYC case for CBU"

    - id: kyc_ready_for_decision
      name: "Ready for KYC Decision"
      description: "All prerequisites met for compliance decision"
      evaluation:
        type: all_of
        query: null
        condition: |
          (and (met? ubo_chain_verified)
               (met? ubo_persons_verified)
               (met? screening_hits_resolved)
               (met? docs_validated)
               (met? risk_rating_assigned))
      gap:
        type: not_ready_for_decision
        severity: info
        message_template: "Prerequisites not complete for KYC decision"
      resolution:
        verbs: []
        requires:
          - ubo_chain_verified
          - ubo_persons_verified
          - screening_hits_resolved
          - docs_validated
          - risk_rating_assigned
        auto_resolve: false
        strategy: "Complete all prerequisites first"

  # ===========================================================================
  # CONDITIONAL REQUIREMENTS
  # ===========================================================================
  conditional_requirements:
    # Luxembourg specific
    - condition: (= jurisdiction "LU")
      requirements:
        - id: cssf_registration
          name: "CSSF Registration"
          description: "Luxembourg fund requires CSSF registration doc"
          evaluation:
            type: exists
            query: (kyc.get-document :cbu $cbu_id :type "CSSF_REGISTRATION")
            condition: (doc-collected? result)
          gap:
            type: missing_cssf_registration
            severity: blocking
            message_template: "CSSF registration document required for LU fund"
          resolution:
            verbs: [kyc.request-document]
            auto_resolve: true
            strategy: "Request CSSF registration from fund administrator"

    # Ireland specific
    - condition: (= jurisdiction "IE")
      requirements:
        - id: cbi_authorization
          name: "CBI Authorization"
          description: "Irish fund requires CBI authorization"
          evaluation:
            type: exists
            query: (kyc.get-document :cbu $cbu_id :type "CBI_AUTHORIZATION")
            condition: (doc-collected? result)
          gap:
            type: missing_cbi_authorization
            severity: blocking
            message_template: "CBI authorization required for IE fund"
          resolution:
            verbs: [kyc.request-document]
            auto_resolve: true

    # High risk jurisdiction
    - condition: (high-risk-jurisdiction? jurisdiction)
      requirements:
        - id: source_of_funds
          name: "Source of Funds"
          description: "Enhanced due diligence - source of funds"
          evaluation:
            type: exists
            query: (kyc.get-document :cbu $cbu_id :type "SOURCE_OF_FUNDS")
            condition: (doc-collected? result)
          gap:
            type: missing_source_of_funds
            severity: blocking
            message_template: "Source of funds required for high-risk jurisdiction"
          resolution:
            verbs: [kyc.request-document]
            auto_resolve: true

        - id: enhanced_screening
          name: "Enhanced Screening"
          description: "Additional screening for high-risk jurisdiction"
          evaluation:
            type: exists
            query: (screening.get :cbu $cbu_id :type ENHANCED)
            condition: (screening-complete? result)
          gap:
            type: missing_enhanced_screening
            severity: blocking
            message_template: "Enhanced screening required"
          resolution:
            verbs: [screening.enhanced]
            auto_resolve: true

    # PEP identified
    - condition: (has-pep? cbu)
      requirements:
        - id: pep_edd
          name: "PEP Enhanced Due Diligence"
          description: "Additional due diligence for PEP relationships"
          evaluation:
            type: exists
            query: (kyc.get-edd :cbu $cbu_id :reason "PEP")
            condition: (edd-complete? result)
          gap:
            type: missing_pep_edd
            severity: blocking
            message_template: "PEP enhanced due diligence required"
          resolution:
            verbs: [kyc.initiate-edd]
            auto_resolve: false
            strategy: "Human review required for PEP relationships"
```

### 1.4 Tasks - End State Model

- [ ] Create `rust/config/end_states/` directory
- [ ] Create `schema.yaml` with end state schema
- [ ] Create `kyc_ready.yaml` with full KYC requirements
- [ ] Create parser for end state YAML
- [ ] Create `EndState` struct in Rust
- [ ] Create `Requirement` struct with evaluation logic
- [ ] Create `Gap` struct with resolution hints
- [ ] Unit tests for end state parsing

---

## Part 2: Gap Analysis Engine

### 2.1 Core Types

**File:** `rust/src/agent/gap_analysis.rs`

```rust
//! Gap Analysis Engine
//!
//! Evaluates current state against end state requirements
//! to identify gaps that need to be closed.

use std::collections::HashMap;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

/// Result of evaluating an end state against current state
#[derive(Debug, Clone, Serialize)]
pub struct GapAnalysis {
    /// The end state being evaluated
    pub end_state_id: String,
    
    /// Entity being evaluated
    pub entity_id: Uuid,
    
    /// Overall progress (0.0 - 1.0)
    pub progress: f32,
    
    /// All requirements with their status
    pub requirements: Vec<RequirementStatus>,
    
    /// Only the gaps (unmet requirements)
    pub gaps: Vec<Gap>,
    
    /// Blocking gaps (must be resolved to proceed)
    pub blocking_gaps: Vec<Gap>,
    
    /// Suggested next actions (prioritized)
    pub suggested_actions: Vec<SuggestedAction>,
    
    /// Timestamp of analysis
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
}

/// Status of a single requirement
#[derive(Debug, Clone, Serialize)]
pub struct RequirementStatus {
    pub requirement_id: String,
    pub name: String,
    pub status: EvaluationResult,
    pub details: Option<String>,
    pub evidence: Vec<String>,
}

/// Result of evaluating a requirement
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum EvaluationResult {
    /// Requirement fully met
    Met,
    /// Requirement partially met (e.g., 3 of 5 docs collected)
    Partial { progress: f32 },
    /// Requirement not met
    NotMet,
    /// Cannot evaluate (missing prerequisite)
    Blocked,
    /// Not applicable (conditional requirement, condition not met)
    NotApplicable,
}

/// A gap that needs to be closed
#[derive(Debug, Clone, Serialize)]
pub struct Gap {
    /// Which requirement is not met
    pub requirement_id: String,
    pub requirement_name: String,
    
    /// Gap classification
    pub gap_type: String,
    pub severity: GapSeverity,
    
    /// Human readable description
    pub message: String,
    
    /// What's specifically missing
    pub details: GapDetails,
    
    /// How to resolve
    pub resolution: ResolutionHints,
    
    /// Prerequisites that must be met first
    pub blocked_by: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum GapSeverity {
    /// Must be resolved to reach end state
    Blocking,
    /// Should be resolved but not strictly required
    Warning,
    /// Informational only
    Info,
}

/// Details about what's missing
#[derive(Debug, Clone, Serialize)]
pub struct GapDetails {
    /// Type of detail
    pub detail_type: GapDetailType,
    /// Specific items (entity IDs, document types, etc.)
    pub items: Vec<GapItem>,
    /// Counts
    pub total_required: Option<usize>,
    pub currently_have: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub enum GapDetailType {
    MissingEntities,
    MissingDocuments,
    MissingScreening,
    MissingEvidence,
    UnresolvedHits,
    InvalidData,
    Custom(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct GapItem {
    pub id: Option<Uuid>,
    pub name: String,
    pub item_type: String,
    pub details: HashMap<String, serde_json::Value>,
}

/// Hints for how to resolve a gap
#[derive(Debug, Clone, Serialize)]
pub struct ResolutionHints {
    /// Verbs that can address this gap
    pub applicable_verbs: Vec<VerbHint>,
    /// Can agent auto-resolve?
    pub auto_resolvable: bool,
    /// Strategy description for agent
    pub strategy: String,
    /// Estimated effort
    pub estimated_actions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerbHint {
    pub verb: String,
    pub suggested_args: HashMap<String, serde_json::Value>,
    pub description: String,
}

/// Suggested action for agent to take
#[derive(Debug, Clone, Serialize)]
pub struct SuggestedAction {
    /// Priority (lower = do first)
    pub priority: u32,
    /// The DSL to execute
    pub dsl: String,
    /// Human description
    pub description: String,
    /// Which gap this addresses
    pub addresses_gap: String,
    /// Can be auto-executed?
    pub auto_execute: bool,
    /// Estimated impact on progress
    pub progress_impact: f32,
}

/// The Gap Analysis Engine
pub struct GapAnalysisEngine {
    /// Loaded end state definitions
    end_states: HashMap<String, EndStateDefinition>,
}

impl GapAnalysisEngine {
    pub fn new() -> Self {
        Self {
            end_states: HashMap::new(),
        }
    }

    /// Load end state definitions from config
    pub fn load_end_states(&mut self, config_dir: &Path) -> Result<(), ConfigError> {
        // Load all YAML files from config/end_states/
        todo!()
    }

    /// Analyze entity against an end state
    pub async fn analyze(
        &self,
        end_state_id: &str,
        entity_id: Uuid,
        context: &ExecutionContext,
    ) -> Result<GapAnalysis, AnalysisError> {
        let end_state = self.end_states.get(end_state_id)
            .ok_or(AnalysisError::UnknownEndState(end_state_id.to_string()))?;

        // 1. Get current entity state
        let entity_state = self.fetch_entity_state(entity_id, context).await?;

        // 2. Evaluate each requirement
        let mut requirements = Vec::new();
        let mut gaps = Vec::new();

        for req in &end_state.requirements {
            // Check if conditional requirement applies
            if let Some(condition) = &req.condition {
                if !self.evaluate_condition(condition, &entity_state)? {
                    requirements.push(RequirementStatus {
                        requirement_id: req.id.clone(),
                        name: req.name.clone(),
                        status: EvaluationResult::NotApplicable,
                        details: None,
                        evidence: vec![],
                    });
                    continue;
                }
            }

            // Check prerequisites
            let blocked_by: Vec<String> = req.resolution.requires.iter()
                .filter(|prereq_id| {
                    requirements.iter()
                        .find(|r| &r.requirement_id == *prereq_id)
                        .map(|r| r.status != EvaluationResult::Met)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            if !blocked_by.is_empty() {
                requirements.push(RequirementStatus {
                    requirement_id: req.id.clone(),
                    name: req.name.clone(),
                    status: EvaluationResult::Blocked,
                    details: Some(format!("Blocked by: {}", blocked_by.join(", "))),
                    evidence: vec![],
                });
                continue;
            }

            // Evaluate requirement
            let (status, details, evidence) = self.evaluate_requirement(
                req, 
                &entity_state, 
                context
            ).await?;

            requirements.push(RequirementStatus {
                requirement_id: req.id.clone(),
                name: req.name.clone(),
                status,
                details: details.clone(),
                evidence,
            });

            // If not met, create gap
            if status != EvaluationResult::Met {
                gaps.push(self.create_gap(req, &status, &details, &entity_state)?);
            }
        }

        // 3. Calculate progress
        let met_count = requirements.iter()
            .filter(|r| r.status == EvaluationResult::Met)
            .count();
        let applicable_count = requirements.iter()
            .filter(|r| r.status != EvaluationResult::NotApplicable)
            .count();
        let progress = if applicable_count > 0 {
            met_count as f32 / applicable_count as f32
        } else {
            0.0
        };

        // 4. Identify blocking gaps
        let blocking_gaps: Vec<Gap> = gaps.iter()
            .filter(|g| g.severity == GapSeverity::Blocking)
            .cloned()
            .collect();

        // 5. Generate suggested actions
        let suggested_actions = self.generate_suggestions(&gaps, &entity_state)?;

        Ok(GapAnalysis {
            end_state_id: end_state_id.to_string(),
            entity_id,
            progress,
            requirements,
            gaps,
            blocking_gaps,
            suggested_actions,
            analyzed_at: chrono::Utc::now(),
        })
    }

    /// Generate prioritized action suggestions
    fn generate_suggestions(
        &self,
        gaps: &[Gap],
        entity_state: &EntityState,
    ) -> Result<Vec<SuggestedAction>, AnalysisError> {
        let mut actions = Vec::new();
        let mut priority = 0u32;

        // Sort gaps: blocking first, then by dependency order
        let mut sorted_gaps = gaps.to_vec();
        sorted_gaps.sort_by(|a, b| {
            // Blocking before non-blocking
            let severity_order = match (&a.severity, &b.severity) {
                (GapSeverity::Blocking, GapSeverity::Blocking) => std::cmp::Ordering::Equal,
                (GapSeverity::Blocking, _) => std::cmp::Ordering::Less,
                (_, GapSeverity::Blocking) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };
            
            if severity_order != std::cmp::Ordering::Equal {
                return severity_order;
            }

            // Then by blocked_by count (fewer blockers = do first)
            a.blocked_by.len().cmp(&b.blocked_by.len())
        });

        for gap in sorted_gaps {
            // Skip gaps that are blocked
            if !gap.blocked_by.is_empty() {
                continue;
            }

            for verb_hint in &gap.resolution.applicable_verbs {
                actions.push(SuggestedAction {
                    priority,
                    dsl: self.generate_dsl(&verb_hint, entity_state)?,
                    description: verb_hint.description.clone(),
                    addresses_gap: gap.requirement_id.clone(),
                    auto_execute: gap.resolution.auto_resolvable,
                    progress_impact: 0.0, // Calculate based on gap weight
                });
                priority += 1;
            }
        }

        Ok(actions)
    }

    fn generate_dsl(
        &self,
        verb_hint: &VerbHint,
        entity_state: &EntityState,
    ) -> Result<String, AnalysisError> {
        // Generate DSL string from verb hint and entity state
        let mut parts = vec![format!("({}", verb_hint.verb)];
        
        for (arg, value) in &verb_hint.suggested_args {
            let value_str = match value {
                serde_json::Value::String(s) => format!("\"{}\"", s),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => value.to_string(),
            };
            parts.push(format!(":{} {}", arg, value_str));
        }
        
        parts.push(")".to_string());
        Ok(parts.join(" "))
    }
}
```

### 2.2 Tasks - Gap Analysis

- [ ] Create `rust/src/agent/` module directory
- [ ] Create `rust/src/agent/mod.rs`
- [ ] Create `rust/src/agent/gap_analysis.rs`
- [ ] Implement `GapAnalysis` struct
- [ ] Implement `GapAnalysisEngine`
- [ ] Implement requirement evaluation
- [ ] Implement gap creation
- [ ] Implement suggestion generation
- [ ] Implement DSL generation from hints
- [ ] Unit tests for gap analysis

---

## Part 3: Agent Orchestration

### 3.1 Agent DSL Verbs

**File:** `rust/config/verbs/agent.yaml`

```yaml
domains:
  agent:
    description: Agent orchestration and gap analysis verbs
    verbs:
      analyze:
        description: Analyze entity against end state, return gaps
        behavior: plugin
        plugin: agent_analyze
        args:
          - name: entity
            type: uuid
            required: true
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: end-state
            type: string
            required: true
            valid_values:
              - kyc_ready
              - ubo_complete
              - docs_collected
              - screening_complete
        returns:
          type: gap_analysis
          description: Full gap analysis with suggestions

      status:
        description: Get current status toward end state
        behavior: plugin
        plugin: agent_status
        args:
          - name: entity
            type: uuid
            required: true
            lookup:
              table: entities
              schema: ob-poc
              search_key: name
              primary_key: entity_id
          - name: end-state
            type: string
            required: false
            default: kyc_ready
        returns:
          type: status_summary

      gaps:
        description: Get only the gaps (blocking issues)
        behavior: plugin
        plugin: agent_gaps
        args:
          - name: entity
            type: uuid
            required: true
          - name: end-state
            type: string
            required: false
            default: kyc_ready
          - name: severity
            type: string
            required: false
            valid_values:
              - blocking
              - warning
              - all
            default: blocking
        returns:
          type: gap_list

      next-actions:
        description: Get suggested next actions to close gaps
        behavior: plugin
        plugin: agent_next_actions
        args:
          - name: entity
            type: uuid
            required: true
          - name: end-state
            type: string
            required: false
            default: kyc_ready
          - name: limit
            type: integer
            required: false
            default: 5
          - name: auto-only
            type: boolean
            required: false
            default: false
            description: Only return actions that can be auto-executed
        returns:
          type: action_list

      execute-plan:
        description: Execute a sequence of actions to close gaps
        behavior: plugin
        plugin: agent_execute_plan
        args:
          - name: entity
            type: uuid
            required: true
          - name: end-state
            type: string
            required: false
            default: kyc_ready
          - name: auto-only
            type: boolean
            required: false
            default: true
            description: Only execute auto-resolvable actions
          - name: dry-run
            type: boolean
            required: false
            default: false
            description: Show what would be done without executing
          - name: max-actions
            type: integer
            required: false
            default: 10
        returns:
          type: execution_result

      close-gap:
        description: Attempt to close a specific gap
        behavior: plugin
        plugin: agent_close_gap
        args:
          - name: entity
            type: uuid
            required: true
          - name: gap-id
            type: string
            required: true
          - name: auto-only
            type: boolean
            required: false
            default: true
        returns:
          type: gap_resolution_result
```

### 3.2 Agent Executor

**File:** `rust/src/agent/executor.rs`

```rust
//! Agent Executor
//!
//! Executes agent orchestration verbs - analyze, plan, execute.

use super::gap_analysis::{GapAnalysis, GapAnalysisEngine, SuggestedAction};
use crate::dsl_v2::DslEngine;

/// Agent executor - bridges gap analysis to DSL execution
pub struct AgentExecutor {
    gap_engine: GapAnalysisEngine,
    dsl_engine: DslEngine,
}

impl AgentExecutor {
    pub fn new(gap_engine: GapAnalysisEngine, dsl_engine: DslEngine) -> Self {
        Self { gap_engine, dsl_engine }
    }

    /// Analyze entity against end state
    pub async fn analyze(
        &self,
        entity_id: Uuid,
        end_state: &str,
        context: &ExecutionContext,
    ) -> Result<GapAnalysis, AgentError> {
        self.gap_engine.analyze(end_state, entity_id, context).await
            .map_err(|e| AgentError::AnalysisFailed(e.to_string()))
    }

    /// Get next actions to close gaps
    pub async fn next_actions(
        &self,
        entity_id: Uuid,
        end_state: &str,
        limit: usize,
        auto_only: bool,
        context: &ExecutionContext,
    ) -> Result<Vec<SuggestedAction>, AgentError> {
        let analysis = self.analyze(entity_id, end_state, context).await?;
        
        let mut actions = analysis.suggested_actions;
        
        if auto_only {
            actions.retain(|a| a.auto_execute);
        }
        
        actions.truncate(limit);
        
        Ok(actions)
    }

    /// Execute a plan to close gaps
    pub async fn execute_plan(
        &self,
        entity_id: Uuid,
        end_state: &str,
        auto_only: bool,
        dry_run: bool,
        max_actions: usize,
        context: &ExecutionContext,
    ) -> Result<ExecutionPlanResult, AgentError> {
        let mut results = Vec::new();
        let mut actions_executed = 0;

        loop {
            // Re-analyze after each action (state has changed)
            let analysis = self.analyze(entity_id, end_state, context).await?;

            // Check if done
            if analysis.gaps.is_empty() {
                return Ok(ExecutionPlanResult {
                    status: PlanStatus::Complete,
                    progress: analysis.progress,
                    actions_executed: results,
                    remaining_gaps: vec![],
                });
            }

            // Check if we've hit max actions
            if actions_executed >= max_actions {
                return Ok(ExecutionPlanResult {
                    status: PlanStatus::MaxActionsReached,
                    progress: analysis.progress,
                    actions_executed: results,
                    remaining_gaps: analysis.gaps,
                });
            }

            // Get next action
            let next_actions = analysis.suggested_actions.iter()
                .filter(|a| !auto_only || a.auto_execute)
                .collect::<Vec<_>>();

            if next_actions.is_empty() {
                return Ok(ExecutionPlanResult {
                    status: if auto_only { 
                        PlanStatus::NeedsHumanIntervention 
                    } else { 
                        PlanStatus::Stuck 
                    },
                    progress: analysis.progress,
                    actions_executed: results,
                    remaining_gaps: analysis.gaps,
                });
            }

            let action = &next_actions[0];

            if dry_run {
                results.push(ActionResult {
                    dsl: action.dsl.clone(),
                    description: action.description.clone(),
                    status: ActionStatus::DryRun,
                    error: None,
                    gap_addressed: action.addresses_gap.clone(),
                });
                actions_executed += 1;
                continue;
            }

            // Execute the action
            match self.dsl_engine.execute(&action.dsl, context).await {
                Ok(result) => {
                    results.push(ActionResult {
                        dsl: action.dsl.clone(),
                        description: action.description.clone(),
                        status: ActionStatus::Success,
                        error: None,
                        gap_addressed: action.addresses_gap.clone(),
                    });
                }
                Err(e) => {
                    results.push(ActionResult {
                        dsl: action.dsl.clone(),
                        description: action.description.clone(),
                        status: ActionStatus::Failed,
                        error: Some(e.to_string()),
                        gap_addressed: action.addresses_gap.clone(),
                    });
                    
                    // Don't continue on failure
                    return Ok(ExecutionPlanResult {
                        status: PlanStatus::ActionFailed,
                        progress: analysis.progress,
                        actions_executed: results,
                        remaining_gaps: analysis.gaps,
                    });
                }
            }

            actions_executed += 1;
        }
    }

    /// Close a specific gap
    pub async fn close_gap(
        &self,
        entity_id: Uuid,
        gap_id: &str,
        auto_only: bool,
        context: &ExecutionContext,
    ) -> Result<GapResolutionResult, AgentError> {
        // Analyze to find the gap
        let analysis = self.analyze(entity_id, "kyc_ready", context).await?;
        
        let gap = analysis.gaps.iter()
            .find(|g| g.requirement_id == gap_id)
            .ok_or(AgentError::GapNotFound(gap_id.to_string()))?;

        if auto_only && !gap.resolution.auto_resolvable {
            return Ok(GapResolutionResult {
                gap_id: gap_id.to_string(),
                status: ResolutionStatus::RequiresHuman,
                actions_taken: vec![],
                message: gap.resolution.strategy.clone(),
            });
        }

        // Execute actions to close this gap
        let mut actions_taken = Vec::new();
        
        for verb_hint in &gap.resolution.applicable_verbs {
            let dsl = self.generate_dsl(verb_hint, entity_id)?;
            
            match self.dsl_engine.execute(&dsl, context).await {
                Ok(_) => {
                    actions_taken.push(ActionResult {
                        dsl: dsl.clone(),
                        description: verb_hint.description.clone(),
                        status: ActionStatus::Success,
                        error: None,
                        gap_addressed: gap_id.to_string(),
                    });
                }
                Err(e) => {
                    actions_taken.push(ActionResult {
                        dsl,
                        description: verb_hint.description.clone(),
                        status: ActionStatus::Failed,
                        error: Some(e.to_string()),
                        gap_addressed: gap_id.to_string(),
                    });
                    break;
                }
            }
        }

        // Re-analyze to see if gap is closed
        let new_analysis = self.analyze(entity_id, "kyc_ready", context).await?;
        let gap_closed = !new_analysis.gaps.iter().any(|g| g.requirement_id == gap_id);

        Ok(GapResolutionResult {
            gap_id: gap_id.to_string(),
            status: if gap_closed { 
                ResolutionStatus::Closed 
            } else { 
                ResolutionStatus::PartialProgress 
            },
            actions_taken,
            message: if gap_closed {
                "Gap closed successfully".to_string()
            } else {
                "Actions executed but gap not fully closed".to_string()
            },
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionPlanResult {
    pub status: PlanStatus,
    pub progress: f32,
    pub actions_executed: Vec<ActionResult>,
    pub remaining_gaps: Vec<Gap>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum PlanStatus {
    Complete,
    MaxActionsReached,
    NeedsHumanIntervention,
    ActionFailed,
    Stuck,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActionResult {
    pub dsl: String,
    pub description: String,
    pub status: ActionStatus,
    pub error: Option<String>,
    pub gap_addressed: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ActionStatus {
    Success,
    Failed,
    DryRun,
}

#[derive(Debug, Clone, Serialize)]
pub struct GapResolutionResult {
    pub gap_id: String,
    pub status: ResolutionStatus,
    pub actions_taken: Vec<ActionResult>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ResolutionStatus {
    Closed,
    PartialProgress,
    RequiresHuman,
    Failed,
}
```

### 3.3 Tasks - Agent Orchestration

- [ ] Create `rust/config/verbs/agent.yaml`
- [ ] Create `rust/src/agent/executor.rs`
- [ ] Implement `AgentExecutor`
- [ ] Implement `execute_plan` with re-evaluation loop
- [ ] Implement `close_gap` for targeted resolution
- [ ] Wire agent verbs to executor
- [ ] Integration tests for agent loop

---

## Part 4: Feedback & Learning

### 4.1 Execution Telemetry

**File:** `rust/src/agent/telemetry.rs`

```rust
//! Agent Telemetry
//!
//! Captures execution data for learning and improvement.

use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Record of a complete agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique execution ID
    pub execution_id: Uuid,
    
    /// Entity being processed
    pub entity_id: Uuid,
    pub entity_type: String,
    pub jurisdiction: Option<String>,
    
    /// End state target
    pub end_state: String,
    
    /// Initial state
    pub initial_progress: f32,
    pub initial_gap_count: usize,
    pub initial_gaps: Vec<String>,
    
    /// Final state
    pub final_progress: f32,
    pub final_gap_count: usize,
    pub final_gaps: Vec<String>,
    
    /// Actions taken
    pub actions: Vec<ActionRecord>,
    
    /// Timing
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: i64,
    
    /// Outcome
    pub outcome: ExecutionOutcome,
    pub human_interventions: usize,
    
    /// Context
    pub triggered_by: TriggerType,
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    /// The DSL executed
    pub dsl: String,
    pub verb: String,
    
    /// What gap it addressed
    pub gap_type: String,
    
    /// Timing
    pub started_at: DateTime<Utc>,
    pub duration_ms: i64,
    
    /// Outcome
    pub success: bool,
    pub error: Option<String>,
    
    /// Impact
    pub gap_closed: bool,
    pub progress_delta: f32,
    
    /// Was this auto or human triggered?
    pub auto_executed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    /// Reached end state
    Complete,
    /// Made progress but not complete
    PartialProgress,
    /// Blocked by human-required action
    NeedsHuman,
    /// Failed with error
    Failed,
    /// Cancelled
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TriggerType {
    /// User initiated
    UserRequest,
    /// Document received
    DocumentEvent,
    /// Screening completed
    ScreeningEvent,
    /// Scheduled check
    Scheduled,
    /// API call
    ApiCall,
}

/// Telemetry collector
pub struct TelemetryCollector {
    /// Current execution being tracked
    current: Option<ExecutionRecord>,
    
    /// Storage backend
    store: Box<dyn TelemetryStore>,
}

impl TelemetryCollector {
    /// Start tracking an execution
    pub fn start_execution(
        &mut self,
        entity_id: Uuid,
        entity_type: &str,
        jurisdiction: Option<&str>,
        end_state: &str,
        initial_analysis: &GapAnalysis,
        trigger: TriggerType,
        user_id: Option<&str>,
    ) -> Uuid {
        let execution_id = Uuid::new_v4();
        
        self.current = Some(ExecutionRecord {
            execution_id,
            entity_id,
            entity_type: entity_type.to_string(),
            jurisdiction: jurisdiction.map(|s| s.to_string()),
            end_state: end_state.to_string(),
            initial_progress: initial_analysis.progress,
            initial_gap_count: initial_analysis.gaps.len(),
            initial_gaps: initial_analysis.gaps.iter()
                .map(|g| g.gap_type.clone())
                .collect(),
            final_progress: 0.0,
            final_gap_count: 0,
            final_gaps: vec![],
            actions: vec![],
            started_at: Utc::now(),
            completed_at: Utc::now(),
            duration_ms: 0,
            outcome: ExecutionOutcome::PartialProgress,
            human_interventions: 0,
            triggered_by: trigger,
            user_id: user_id.map(|s| s.to_string()),
        });

        execution_id
    }

    /// Record an action
    pub fn record_action(&mut self, action: ActionRecord) {
        if let Some(ref mut record) = self.current {
            record.actions.push(action);
        }
    }

    /// Complete execution tracking
    pub async fn complete_execution(
        &mut self,
        final_analysis: &GapAnalysis,
        outcome: ExecutionOutcome,
    ) -> Result<(), TelemetryError> {
        if let Some(mut record) = self.current.take() {
            record.final_progress = final_analysis.progress;
            record.final_gap_count = final_analysis.gaps.len();
            record.final_gaps = final_analysis.gaps.iter()
                .map(|g| g.gap_type.clone())
                .collect();
            record.completed_at = Utc::now();
            record.duration_ms = (record.completed_at - record.started_at)
                .num_milliseconds();
            record.outcome = outcome;
            record.human_interventions = record.actions.iter()
                .filter(|a| !a.auto_executed)
                .count();

            self.store.save(record).await?;
        }
        Ok(())
    }
}

/// Trait for telemetry storage backends
#[async_trait]
pub trait TelemetryStore: Send + Sync {
    async fn save(&self, record: ExecutionRecord) -> Result<(), TelemetryError>;
    async fn query(&self, query: TelemetryQuery) -> Result<Vec<ExecutionRecord>, TelemetryError>;
    async fn get_stats(&self, query: StatsQuery) -> Result<ExecutionStats, TelemetryError>;
}
```

### 4.2 Learning Analytics

**File:** `rust/src/agent/analytics.rs`

```rust
//! Agent Learning Analytics
//!
//! Analyzes telemetry to improve agent performance.

/// Aggregated statistics from telemetry
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionStats {
    /// Total executions
    pub total_executions: usize,
    
    /// Outcome breakdown
    pub outcomes: HashMap<ExecutionOutcome, usize>,
    
    /// Average progress improvement
    pub avg_progress_improvement: f32,
    
    /// Average actions to completion
    pub avg_actions_to_complete: f32,
    
    /// Average duration
    pub avg_duration_ms: i64,
    
    /// Gap resolution rates
    pub gap_resolution_rates: HashMap<String, GapStats>,
    
    /// Verb effectiveness
    pub verb_effectiveness: HashMap<String, VerbStats>,
    
    /// Patterns by entity type
    pub entity_type_patterns: HashMap<String, EntityTypePattern>,
    
    /// Patterns by jurisdiction
    pub jurisdiction_patterns: HashMap<String, JurisdictionPattern>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GapStats {
    pub gap_type: String,
    pub occurrences: usize,
    pub auto_resolved: usize,
    pub human_resolved: usize,
    pub avg_resolution_time_ms: i64,
    pub common_verbs: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerbStats {
    pub verb: String,
    pub total_uses: usize,
    pub success_rate: f32,
    pub avg_duration_ms: i64,
    pub gaps_addressed: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityTypePattern {
    pub entity_type: String,
    pub avg_gap_count: f32,
    pub common_gaps: Vec<(String, f32)>,  // (gap_type, frequency)
    pub typical_verb_sequence: Vec<String>,
    pub avg_time_to_complete_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct JurisdictionPattern {
    pub jurisdiction: String,
    pub additional_requirements: Vec<String>,
    pub avg_doc_count: f32,
    pub common_issues: Vec<String>,
}

/// Analytics engine
pub struct AnalyticsEngine {
    store: Box<dyn TelemetryStore>,
}

impl AnalyticsEngine {
    /// Get recommendations for a specific entity
    pub async fn get_recommendations(
        &self,
        entity_type: &str,
        jurisdiction: Option<&str>,
        current_gaps: &[Gap],
    ) -> Result<Vec<Recommendation>, AnalyticsError> {
        // Look up patterns for this entity type / jurisdiction
        let patterns = self.get_patterns(entity_type, jurisdiction).await?;

        let mut recommendations = Vec::new();

        // Predict likely future gaps
        for (gap_type, frequency) in &patterns.common_gaps {
            if frequency > &0.5 && !current_gaps.iter().any(|g| &g.gap_type == gap_type) {
                recommendations.push(Recommendation {
                    rec_type: RecommendationType::PreemptiveAction,
                    message: format!(
                        "{}% of {} entities encounter '{}' - consider addressing proactively",
                        (frequency * 100.0) as u32,
                        entity_type,
                        gap_type
                    ),
                    suggested_action: None,
                    confidence: *frequency,
                });
            }
        }

        // Recommend optimal verb sequence
        if !patterns.typical_verb_sequence.is_empty() {
            recommendations.push(Recommendation {
                rec_type: RecommendationType::OptimalSequence,
                message: "Recommended action sequence based on historical success".to_string(),
                suggested_action: Some(patterns.typical_verb_sequence.clone()),
                confidence: 0.8,
            });
        }

        // Flag jurisdiction-specific requirements
        if let Some(j) = jurisdiction {
            if let Some(jp) = self.get_jurisdiction_pattern(j).await? {
                for req in &jp.additional_requirements {
                    recommendations.push(Recommendation {
                        rec_type: RecommendationType::JurisdictionRequirement,
                        message: format!("{} requires: {}", j, req),
                        suggested_action: None,
                        confidence: 1.0,
                    });
                }
            }
        }

        Ok(recommendations)
    }

    /// Identify areas for improvement
    pub async fn get_improvement_insights(&self) -> Result<Vec<Insight>, AnalyticsError> {
        let stats = self.get_stats(StatsQuery::default()).await?;
        let mut insights = Vec::new();

        // Find gaps with low auto-resolution rate
        for (gap_type, gap_stats) in &stats.gap_resolution_rates {
            let auto_rate = if gap_stats.occurrences > 0 {
                gap_stats.auto_resolved as f32 / gap_stats.occurrences as f32
            } else {
                0.0
            };

            if auto_rate < 0.5 && gap_stats.occurrences > 10 {
                insights.push(Insight {
                    insight_type: InsightType::LowAutoResolution,
                    message: format!(
                        "'{}' has only {:.0}% auto-resolution rate ({} occurrences)",
                        gap_type,
                        auto_rate * 100.0,
                        gap_stats.occurrences
                    ),
                    recommendation: "Consider adding more resolution strategies".to_string(),
                    impact: InsightImpact::High,
                });
            }
        }

        // Find verbs with low success rate
        for (verb, verb_stats) in &stats.verb_effectiveness {
            if verb_stats.success_rate < 0.8 && verb_stats.total_uses > 20 {
                insights.push(Insight {
                    insight_type: InsightType::VerbFailures,
                    message: format!(
                        "'{}' has {:.0}% success rate ({} uses)",
                        verb,
                        verb_stats.success_rate * 100.0,
                        verb_stats.total_uses
                    ),
                    recommendation: "Investigate common failure causes".to_string(),
                    impact: InsightImpact::Medium,
                });
            }
        }

        Ok(insights)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub rec_type: RecommendationType,
    pub message: String,
    pub suggested_action: Option<Vec<String>>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum RecommendationType {
    PreemptiveAction,
    OptimalSequence,
    JurisdictionRequirement,
    RiskWarning,
}

#[derive(Debug, Clone, Serialize)]
pub struct Insight {
    pub insight_type: InsightType,
    pub message: String,
    pub recommendation: String,
    pub impact: InsightImpact,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum InsightType {
    LowAutoResolution,
    VerbFailures,
    SlowResolution,
    CommonBlocker,
    PatternAnomaly,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum InsightImpact {
    High,
    Medium,
    Low,
}
```

### 4.3 Tasks - Feedback & Learning

- [ ] Create `rust/src/agent/telemetry.rs`
- [ ] Implement `ExecutionRecord` struct
- [ ] Implement `TelemetryCollector`
- [ ] Implement database storage for telemetry
- [ ] Create `rust/src/agent/analytics.rs`
- [ ] Implement `ExecutionStats` aggregation
- [ ] Implement pattern detection
- [ ] Implement recommendation generation
- [ ] Create analytics dashboard queries

---

## Part 5: LLM Agent Integration

### 5.1 Agent System Prompt

**File:** `rust/config/agent/kyc_agent_prompt.md`

```markdown
# KYC Agent System Prompt

You are a KYC (Know Your Customer) specialist agent for a financial services 
onboarding platform. Your role is to help complete KYC requirements for 
Client Business Units (CBUs) by analyzing gaps and executing appropriate 
actions via DSL verbs.

## Your Capabilities

You have access to the following DSL domains:

### Analysis
- `(agent.analyze :entity @cbu :end-state "kyc_ready")` - Analyze gaps
- `(agent.status :entity @cbu)` - Get current status
- `(agent.gaps :entity @cbu)` - Get blocking gaps
- `(agent.next-actions :entity @cbu)` - Get suggested actions

### Entity Management
- `(entity.create-* ...)` - Create entities (funds, companies, persons)
- `(entity.update ...)` - Update entity attributes

### UBO Chain
- `(ubo.add-ownership ...)` - Add ownership relationship
- `(ubo.add-control ...)` - Add control person
- `(ubo.apply-exemption ...)` - Apply listed company/regulated fund exemption
- `(ubo.verify-chain ...)` - Verify ownership chain

### KYC Documents
- `(kyc.request-document ...)` - Request document from client
- `(kyc.link-evidence ...)` - Link document as evidence
- `(kyc.validate-document ...)` - Validate document

### Screening
- `(screening.pep :person @person)` - Run PEP screening
- `(screening.sanctions :entity @entity)` - Run sanctions screening
- `(screening.adverse-media :entity @entity)` - Run adverse media check

### Graph Visualization
- `(graph.view :focus @entity ...)` - Show entity in context

## Your Process

1. **Assess**: Always start by analyzing the current state
   - Run `(agent.analyze ...)` to understand gaps
   - Use `(graph.view ...)` to visualize the structure

2. **Plan**: Identify which gaps to address
   - Prioritize blocking gaps
   - Consider dependencies (some gaps must be closed before others)
   - Check if actions can be auto-executed or need human input

3. **Execute**: Take actions to close gaps
   - Execute DSL verbs
   - Verify the action succeeded
   - Re-analyze to confirm gap is closed

4. **Report**: Communicate clearly with the user
   - Explain what you did and why
   - Show progress toward end state
   - Highlight anything that needs human attention

## Domain Knowledge

### UBO Requirements
- 25% ownership threshold (EU 4AMLD/5AMLD)
- Must trace to natural persons OR apply exemption
- Control prong: directors, officers with significant control
- Each ownership link needs documentary evidence

### Exemptions
- **Listed Company**: Publicly traded on regulated exchange
- **Regulated Fund**: UCITS, AIF under regulatory supervision
- When exemption applies, no need to identify natural person UBOs

### Document Requirements by Entity Type
- **Fund**: Prospectus, KID, regulatory registration
- **Company**: Certificate of incorporation, register of directors, 
  register of shareholders
- **Natural Person**: Passport/ID, proof of address
- **Trust**: Trust deed, schedule of beneficiaries

### Jurisdiction-Specific
- **Luxembourg (LU)**: CSSF registration required
- **Ireland (IE)**: CBI authorization required
- **High-risk jurisdictions**: Enhanced due diligence, source of funds

### Screening
- All natural persons: PEP + sanctions + adverse media
- All legal entities: Sanctions
- Hits require human review before proceeding

## Communication Style

- Be concise but thorough
- Always show the DSL you're executing
- Explain your reasoning
- Highlight decisions that need human input
- Use the graph visualization to help users understand structure

## Example Interaction

User: "Start KYC for Allianz Income Fund"

Agent: Let me analyze the current state.

```
(agent.analyze :entity @allianz-income-fund :end-state "kyc_ready")
```

Current status: 35% complete

**Blocking Gaps:**
1. UBO chain incomplete - need to trace ownership to natural persons
2. Control persons not identified - need register of directors
3. Screening not started - 0 persons screened

Let me show you the current structure:

```
(graph.view :focus @allianz-income-fund :depth 3 :edges [ownership])
```

The fund is owned by Allianz GI Luxembourg (ManCo), which is owned by 
Allianz Global Investors GmbH (DE). I can see Allianz SE at the top of 
the chain.

Allianz SE is publicly traded on XETRA, so the listed company exemption 
applies. I'll apply that:

```
(ubo.apply-exemption :entity @allianz-se :type "LISTED_COMPANY" 
                     :evidence "XETRA:ALV")
```

UBO chain is now complete. Next, I need the register of directors 
from the ManCo to identify control persons:

```
(kyc.request-document :entity @allianz-gi-lu 
                      :type "REGISTER_OF_DIRECTORS"
                      :reason "Control prong - identify officers")
```

Document requested. Once received, I'll:
1. Extract director names
2. Create person entities
3. Run PEP/sanctions screening

Current progress: 55% complete. Waiting for document.
```

### 5.2 Tasks - LLM Integration

- [ ] Create `rust/config/agent/kyc_agent_prompt.md`
- [ ] Create agent API endpoint (accepts natural language, returns DSL + results)
- [ ] Implement conversation context management
- [ ] Implement DSL extraction from LLM response
- [ ] Implement execution + response formatting
- [ ] Add agent UI component (chat interface)

---

## Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  IMPLEMENTATION ORDER                                                       │
│                                                                             │
│  1. Part 1: End State Model (YAML schema + kyc_ready definition)           │
│  2. Part 2: Gap Analysis Engine (evaluate requirements, identify gaps)     │
│  3. Part 3: Agent Orchestration (agent.* verbs, executor)                  │
│  4. Part 4: Telemetry & Learning (execution records, analytics)            │
│  5. Part 5: LLM Integration (system prompt, API, UI)                       │
│                                                                             │
│  ESTIMATED: 5-7 days                                                        │
│                                                                             │
│  DEPENDENCIES:                                                              │
│  - Existing DSL infrastructure                                             │
│  - Graph DSL domain (TODO-GRAPH-DSL-DOMAIN.md)                             │
│  - KYC verbs (existing)                                                    │
│  - Screening verbs (existing)                                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Vision Realized

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  User: "Get this fund KYC ready"                                           │
│                                                                             │
│  Agent:                                                                     │
│    1. Analyzes current state → identifies 8 gaps                           │
│    2. Applies domain knowledge → GLEIF for ownership, exemptions          │
│    3. Executes verbs → closes 5 gaps automatically                        │
│    4. Reports → "3 items need your input: screening hits, doc review"     │
│    5. Learns → execution data improves future performance                 │
│                                                                             │
│  The agent moves the chess pieces. The human approves the strategy.       │
│                                                                             │
│  Every execution makes the next one better.                                │
│  Intent-driven. Gap-based. Continuously learning.                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Success Criteria

- [ ] End state model parses and validates
- [ ] Gap analysis correctly identifies missing requirements
- [ ] Agent verbs execute and return meaningful results
- [ ] Execute-plan loop closes gaps iteratively
- [ ] Telemetry captures all execution data
- [ ] Analytics provides actionable insights
- [ ] LLM agent can run complete KYC workflow
- [ ] Human intervention points are clear
- [ ] Progress is always visible

---

*Intent → End State → Gaps → Verbs → Learn → Repeat*
