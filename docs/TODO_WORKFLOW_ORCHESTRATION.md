# TODO: Workflow Orchestration Layer

**Purpose**: Stateful workflow engine for KYC, UBO, and onboarding processes  
**Priority**: CRITICAL - Core business process orchestration  
**Effort**: ~12-16 hours

---

## Problem Statement

Current state: We have DSL verbs that do atomic operations. No orchestration layer that:
- Tracks workflow state (where are we in the process?)
- Knows prerequisites (what's needed before we can advance?)
- Enforces transitions (can we move from SCREENING → APPROVED?)
- Coordinates parallel vs sequential steps
- Handles blocking conditions and remediation

**Example**: KYC onboarding isn't just "run these 10 DSL commands". It's:
1. Create CBU → triggers workflow start
2. Add required roles (DIRECTOR, UBO) → blocks until minimum roles met
3. Run screening on each person → can run in parallel
4. Collect documents → blocks until required docs present
5. Calculate UBO → requires ownership structure complete
6. Verify each UBO → blocks until all verified
7. Final review → requires all prior steps complete
8. Approve/Reject → terminal state

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           WORKFLOW ENGINE                                   │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Workflow   │  │   State     │  │ Transition  │  │  Blocker    │        │
│  │ Definition  │  │  Tracker    │  │   Guard     │  │  Resolver   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DSL EXECUTION                                     │
│              Workflow emits DSL → Executor runs → Results fed back          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Core Concepts

### 1.1 Workflow Definition

```yaml
# rust/config/workflows/kyc_onboarding.yaml
workflow: kyc_onboarding
version: 1
description: Full KYC onboarding for a new CBU

# Workflow is triggered by this event
trigger:
  event: cbu.created
  conditions:
    - field: cbu_type
      in: [FUND, CORPORATE, INSTITUTIONAL]

# Define the states
states:
  INTAKE:
    description: Initial data gathering
    initial: true
    
  ENTITY_COLLECTION:
    description: Collecting related entities (directors, UBOs, etc.)
    
  SCREENING:
    description: Running AML/PEP screening on all entities
    
  DOCUMENT_COLLECTION:
    description: Gathering required documentation
    
  UBO_DETERMINATION:
    description: Calculating and verifying beneficial owners
    
  REVIEW:
    description: Analyst review of complete package
    
  APPROVED:
    description: Onboarding complete, ready for business
    terminal: true
    
  REJECTED:
    description: Onboarding rejected
    terminal: true
    
  REMEDIATION:
    description: Issues found, need correction
    
# Define valid transitions
transitions:
  - from: INTAKE
    to: ENTITY_COLLECTION
    auto: true  # Automatic when prerequisites met
    
  - from: ENTITY_COLLECTION
    to: SCREENING
    guard: entities_complete
    
  - from: SCREENING
    to: DOCUMENT_COLLECTION
    guard: screening_complete
    
  - from: DOCUMENT_COLLECTION
    to: UBO_DETERMINATION
    guard: documents_complete
    
  - from: UBO_DETERMINATION
    to: REVIEW
    guard: ubo_complete
    
  - from: REVIEW
    to: APPROVED
    guard: review_approved
    manual: true  # Requires explicit action
    
  - from: REVIEW
    to: REJECTED
    guard: review_rejected
    manual: true
    
  - from: REVIEW
    to: REMEDIATION
    manual: true
    
  - from: REMEDIATION
    to: ENTITY_COLLECTION
    # Can go back to fix issues

# Define what's required at each state
requirements:
  ENTITY_COLLECTION:
    - type: role_count
      role: DIRECTOR
      min: 1
      description: "At least one director required"
      
    - type: role_count
      role: AUTHORIZED_SIGNATORY
      min: 1
      description: "At least one authorized signatory required"
      
  SCREENING:
    - type: all_entities_screened
      description: "All linked entities must have screening results"
      
  DOCUMENT_COLLECTION:
    - type: document_set
      documents:
        - CERTIFICATE_OF_INCORPORATION
        - REGISTER_OF_DIRECTORS
        - REGISTER_OF_SHAREHOLDERS
      description: "Corporate documents required"
      
    - type: per_entity_document
      entity_type: DIRECTOR
      documents:
        - PASSPORT
        - PROOF_OF_ADDRESS
      description: "ID documents for each director"
      
  UBO_DETERMINATION:
    - type: ownership_complete
      threshold: 100  # Total ownership must sum to 100%
      
    - type: all_ubos_verified
      description: "All UBOs must be verified"
      
  REVIEW:
    - type: no_open_alerts
      description: "No unresolved screening alerts"
      
    - type: case_checklist_complete
      description: "All checklist items signed off"

# Actions available at each state
actions:
  ENTITY_COLLECTION:
    - action: add_role
      verb: cbu.assign-role
      description: "Add a person to a role"
      
    - action: add_ownership
      verb: ubo.add-ownership
      description: "Add ownership link"
      
  SCREENING:
    - action: run_screening
      verb: screening.run
      description: "Run screening on entity"
      
    - action: clear_alert
      verb: screening.clear-alert
      description: "Clear a screening alert"
      
  DOCUMENT_COLLECTION:
    - action: upload_document
      verb: document.upload
      description: "Upload a document"
      
    - action: verify_document
      verb: document.verify
      description: "Verify document authenticity"
      
  UBO_DETERMINATION:
    - action: calculate_ubo
      verb: ubo.calculate
      description: "Calculate beneficial owners"
      
    - action: verify_ubo
      verb: ubo.verify-ubo
      description: "Verify a beneficial owner"
```

### 1.2 Workflow Instance State

```rust
// rust/src/workflow/state.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A running instance of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    pub instance_id: Uuid,
    pub workflow_id: String,          // "kyc_onboarding"
    pub version: u32,
    
    // What entity this workflow is for
    pub subject_type: String,         // "cbu"
    pub subject_id: Uuid,
    
    // Current state
    pub current_state: String,        // "SCREENING"
    pub state_entered_at: DateTime<Utc>,
    
    // State history
    pub history: Vec<StateTransition>,
    
    // Computed blockers (why can't we advance?)
    pub blockers: Vec<Blocker>,
    
    // Metadata
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_state: String,
    pub to_state: String,
    pub transitioned_at: DateTime<Utc>,
    pub transitioned_by: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    pub blocker_type: BlockerType,
    pub description: String,
    pub resolution_action: Option<String>,  // DSL verb that resolves this
    pub details: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerType {
    MissingRole { role: String, required: u32, current: u32 },
    MissingDocument { document_type: String, for_entity: Option<Uuid> },
    PendingScreening { entity_id: Uuid },
    UnresolvedAlert { alert_id: Uuid, entity_id: Uuid },
    IncompleteOwnership { current_total: f64, required: f64 },
    UnverifiedUbo { ubo_id: Uuid, person_name: String },
    ManualApprovalRequired,
    Custom { code: String },
}
```

---

## Part 2: Guard Evaluation

Guards determine if a transition is allowed:

```rust
// rust/src/workflow/guards.rs

use sqlx::PgPool;
use uuid::Uuid;

/// Evaluates transition guards
pub struct GuardEvaluator {
    pool: PgPool,
}

impl GuardEvaluator {
    pub async fn evaluate(
        &self,
        guard_name: &str,
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<GuardResult, sqlx::Error> {
        match guard_name {
            "entities_complete" => self.check_entities_complete(subject_id).await,
            "screening_complete" => self.check_screening_complete(subject_id).await,
            "documents_complete" => self.check_documents_complete(subject_id).await,
            "ubo_complete" => self.check_ubo_complete(subject_id).await,
            "review_approved" => self.check_review_approved(subject_id).await,
            "review_rejected" => self.check_review_rejected(subject_id).await,
            _ => Ok(GuardResult::failed(format!("Unknown guard: {}", guard_name))),
        }
    }
    
    async fn check_entities_complete(&self, cbu_id: Uuid) -> Result<GuardResult, sqlx::Error> {
        // Check minimum role requirements
        let director_count: i64 = sqlx::query_scalar(r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_roles cr
            JOIN "ob-poc".roles r ON cr.role_id = r.role_id
            WHERE cr.cbu_id = $1 
            AND r.role_code = 'DIRECTOR'
            AND cr.effective_to IS NULL
        "#)
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await?;
        
        let mut blockers = Vec::new();
        
        if director_count < 1 {
            blockers.push(Blocker {
                blocker_type: BlockerType::MissingRole {
                    role: "DIRECTOR".to_string(),
                    required: 1,
                    current: director_count as u32,
                },
                description: "At least one director required".to_string(),
                resolution_action: Some("cbu.assign-role".to_string()),
                details: Default::default(),
            });
        }
        
        // Check authorized signatory
        let sig_count: i64 = sqlx::query_scalar(r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_roles cr
            JOIN "ob-poc".roles r ON cr.role_id = r.role_id
            WHERE cr.cbu_id = $1 
            AND r.role_code = 'AUTHORIZED_SIGNATORY'
            AND cr.effective_to IS NULL
        "#)
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await?;
        
        if sig_count < 1 {
            blockers.push(Blocker {
                blocker_type: BlockerType::MissingRole {
                    role: "AUTHORIZED_SIGNATORY".to_string(),
                    required: 1,
                    current: sig_count as u32,
                },
                description: "At least one authorized signatory required".to_string(),
                resolution_action: Some("cbu.assign-role".to_string()),
                details: Default::default(),
            });
        }
        
        if blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(blockers))
        }
    }
    
    async fn check_screening_complete(&self, cbu_id: Uuid) -> Result<GuardResult, sqlx::Error> {
        // Find all linked entities that need screening
        let unscreened: Vec<(Uuid, String)> = sqlx::query_as(r#"
            SELECT e.entity_id, e.display_name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_roles cr ON e.entity_id = cr.entity_id
            WHERE cr.cbu_id = $1
            AND cr.effective_to IS NULL
            AND NOT EXISTS (
                SELECT 1 FROM "ob-poc".screening_results sr
                WHERE sr.entity_id = e.entity_id
                AND sr.screening_date > NOW() - INTERVAL '90 days'
            )
        "#)
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        if unscreened.is_empty() {
            // Also check for unresolved alerts
            let open_alerts: Vec<Uuid> = sqlx::query_scalar(r#"
                SELECT sa.alert_id
                FROM "ob-poc".screening_alerts sa
                JOIN "ob-poc".screening_results sr ON sa.screening_id = sr.screening_id
                JOIN "ob-poc".cbu_roles cr ON sr.entity_id = cr.entity_id
                WHERE cr.cbu_id = $1
                AND sa.status = 'OPEN'
            "#)
            .bind(cbu_id)
            .fetch_all(&self.pool)
            .await?;
            
            if open_alerts.is_empty() {
                return Ok(GuardResult::passed());
            }
            
            let blockers = open_alerts.iter().map(|alert_id| Blocker {
                blocker_type: BlockerType::UnresolvedAlert {
                    alert_id: *alert_id,
                    entity_id: Uuid::nil(),  // Would need to join to get this
                },
                description: "Unresolved screening alert".to_string(),
                resolution_action: Some("screening.clear-alert".to_string()),
                details: Default::default(),
            }).collect();
            
            return Ok(GuardResult::blocked(blockers));
        }
        
        let blockers = unscreened.iter().map(|(id, name)| Blocker {
            blocker_type: BlockerType::PendingScreening { entity_id: *id },
            description: format!("Screening required for {}", name),
            resolution_action: Some("screening.run".to_string()),
            details: [("entity_id".to_string(), serde_json::json!(id))].into(),
        }).collect();
        
        Ok(GuardResult::blocked(blockers))
    }
    
    async fn check_documents_complete(&self, cbu_id: Uuid) -> Result<GuardResult, sqlx::Error> {
        let mut blockers = Vec::new();
        
        // Check CBU-level required documents
        let required_cbu_docs = ["CERTIFICATE_OF_INCORPORATION", "REGISTER_OF_DIRECTORS"];
        for doc_type in required_cbu_docs {
            let exists: bool = sqlx::query_scalar(r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".documents d
                    WHERE d.cbu_id = $1
                    AND d.document_type = $2
                    AND d.status = 'VERIFIED'
                )
            "#)
            .bind(cbu_id)
            .bind(doc_type)
            .fetch_one(&self.pool)
            .await?;
            
            if !exists {
                blockers.push(Blocker {
                    blocker_type: BlockerType::MissingDocument {
                        document_type: doc_type.to_string(),
                        for_entity: None,
                    },
                    description: format!("{} required", doc_type.replace('_', " ").to_lowercase()),
                    resolution_action: Some("document.upload".to_string()),
                    details: Default::default(),
                });
            }
        }
        
        // Check per-director documents
        let directors: Vec<(Uuid, String)> = sqlx::query_as(r#"
            SELECT e.entity_id, e.display_name
            FROM "ob-poc".entities e
            JOIN "ob-poc".cbu_roles cr ON e.entity_id = cr.entity_id
            JOIN "ob-poc".roles r ON cr.role_id = r.role_id
            WHERE cr.cbu_id = $1
            AND r.role_code = 'DIRECTOR'
            AND cr.effective_to IS NULL
        "#)
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        for (director_id, director_name) in directors {
            let has_passport: bool = sqlx::query_scalar(r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".documents d
                    WHERE d.entity_id = $1
                    AND d.document_type = 'PASSPORT'
                    AND d.status = 'VERIFIED'
                )
            "#)
            .bind(director_id)
            .fetch_one(&self.pool)
            .await?;
            
            if !has_passport {
                blockers.push(Blocker {
                    blocker_type: BlockerType::MissingDocument {
                        document_type: "PASSPORT".to_string(),
                        for_entity: Some(director_id),
                    },
                    description: format!("Passport required for {}", director_name),
                    resolution_action: Some("document.upload".to_string()),
                    details: [("entity_id".to_string(), serde_json::json!(director_id))].into(),
                });
            }
        }
        
        if blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(blockers))
        }
    }
    
    async fn check_ubo_complete(&self, cbu_id: Uuid) -> Result<GuardResult, sqlx::Error> {
        let mut blockers = Vec::new();
        
        // Check ownership totals to 100%
        let total_ownership: Option<f64> = sqlx::query_scalar(r#"
            SELECT SUM(percentage) FROM "ob-poc".ownership_links
            WHERE owned_cbu_id = $1
        "#)
        .bind(cbu_id)
        .fetch_one(&self.pool)
        .await?;
        
        let total = total_ownership.unwrap_or(0.0);
        if (total - 100.0).abs() > 0.01 {
            blockers.push(Blocker {
                blocker_type: BlockerType::IncompleteOwnership {
                    current_total: total,
                    required: 100.0,
                },
                description: format!("Ownership structure incomplete ({:.1}% of 100%)", total),
                resolution_action: Some("ubo.add-ownership".to_string()),
                details: Default::default(),
            });
        }
        
        // Check all UBOs are verified
        let unverified_ubos: Vec<(Uuid, String)> = sqlx::query_as(r#"
            SELECT u.ubo_id, e.display_name
            FROM "ob-poc".ubo_registry u
            JOIN "ob-poc".entities e ON u.ubo_person_id = e.entity_id
            WHERE u.cbu_id = $1
            AND u.verification_status NOT IN ('VERIFIED', 'PROVEN')
        "#)
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;
        
        for (ubo_id, name) in unverified_ubos {
            blockers.push(Blocker {
                blocker_type: BlockerType::UnverifiedUbo {
                    ubo_id,
                    person_name: name.clone(),
                },
                description: format!("UBO verification required for {}", name),
                resolution_action: Some("ubo.verify-ubo".to_string()),
                details: [("ubo_id".to_string(), serde_json::json!(ubo_id))].into(),
            });
        }
        
        if blockers.is_empty() {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::blocked(blockers))
        }
    }
    
    async fn check_review_approved(&self, cbu_id: Uuid) -> Result<GuardResult, sqlx::Error> {
        let case_status: Option<String> = sqlx::query_scalar(r#"
            SELECT status FROM "ob-poc".kyc_cases
            WHERE cbu_id = $1
            ORDER BY created_at DESC
            LIMIT 1
        "#)
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?;
        
        match case_status.as_deref() {
            Some("APPROVED") => Ok(GuardResult::passed()),
            _ => Ok(GuardResult::blocked(vec![Blocker {
                blocker_type: BlockerType::ManualApprovalRequired,
                description: "Analyst approval required".to_string(),
                resolution_action: Some("kyc-case.update-status".to_string()),
                details: Default::default(),
            }])),
        }
    }
    
    async fn check_review_rejected(&self, cbu_id: Uuid) -> Result<GuardResult, sqlx::Error> {
        let case_status: Option<String> = sqlx::query_scalar(r#"
            SELECT status FROM "ob-poc".kyc_cases
            WHERE cbu_id = $1
            ORDER BY created_at DESC
            LIMIT 1
        "#)
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await?;
        
        match case_status.as_deref() {
            Some("REJECTED") | Some("DO_NOT_ONBOARD") => Ok(GuardResult::passed()),
            _ => Ok(GuardResult::blocked(vec![])),
        }
    }
}

#[derive(Debug)]
pub struct GuardResult {
    pub passed: bool,
    pub blockers: Vec<Blocker>,
}

impl GuardResult {
    pub fn passed() -> Self {
        Self { passed: true, blockers: vec![] }
    }
    
    pub fn blocked(blockers: Vec<Blocker>) -> Self {
        Self { passed: false, blockers }
    }
    
    pub fn failed(reason: String) -> Self {
        Self {
            passed: false,
            blockers: vec![Blocker {
                blocker_type: BlockerType::Custom { code: "GUARD_ERROR".to_string() },
                description: reason,
                resolution_action: None,
                details: Default::default(),
            }],
        }
    }
}
```

---

## Part 3: Workflow Engine

```rust
// rust/src/workflow/engine.rs

use super::guards::GuardEvaluator;
use super::state::{Blocker, StateTransition, WorkflowInstance};
use super::definition::WorkflowDefinition;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

pub struct WorkflowEngine {
    pool: PgPool,
    guard_evaluator: GuardEvaluator,
    definitions: HashMap<String, WorkflowDefinition>,
}

impl WorkflowEngine {
    /// Start a new workflow instance
    pub async fn start_workflow(
        &self,
        workflow_id: &str,
        subject_type: &str,
        subject_id: Uuid,
        created_by: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let definition = self.definitions.get(workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(workflow_id.to_string()))?;
        
        let initial_state = definition.states.iter()
            .find(|(_, s)| s.initial)
            .map(|(name, _)| name.clone())
            .ok_or_else(|| WorkflowError::NoInitialState)?;
        
        let instance = WorkflowInstance {
            instance_id: Uuid::new_v4(),
            workflow_id: workflow_id.to_string(),
            version: definition.version,
            subject_type: subject_type.to_string(),
            subject_id,
            current_state: initial_state,
            state_entered_at: Utc::now(),
            history: vec![],
            blockers: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by,
        };
        
        // Persist
        self.save_instance(&instance).await?;
        
        // Immediately try to advance (for auto transitions)
        self.try_advance(instance.instance_id).await
    }
    
    /// Get current state and blockers for a workflow
    pub async fn get_status(
        &self,
        instance_id: Uuid,
    ) -> Result<WorkflowStatus, WorkflowError> {
        let instance = self.load_instance(instance_id).await?;
        let definition = self.definitions.get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
        
        // Evaluate current blockers
        let blockers = self.evaluate_blockers(&instance, definition).await?;
        
        // Get available transitions
        let available_transitions = self.get_available_transitions(&instance, definition).await?;
        
        // Get available actions
        let available_actions = definition.get_actions_for_state(&instance.current_state);
        
        Ok(WorkflowStatus {
            instance_id: instance.instance_id,
            workflow_id: instance.workflow_id.clone(),
            subject_id: instance.subject_id,
            current_state: instance.current_state.clone(),
            state_description: definition.states.get(&instance.current_state)
                .map(|s| s.description.clone()),
            is_terminal: definition.states.get(&instance.current_state)
                .map(|s| s.terminal)
                .unwrap_or(false),
            blockers,
            available_transitions,
            available_actions,
            progress: self.calculate_progress(&instance, definition),
            history: instance.history.clone(),
        })
    }
    
    /// Try to automatically advance the workflow
    pub async fn try_advance(
        &self,
        instance_id: Uuid,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let mut instance = self.load_instance(instance_id).await?;
        let definition = self.definitions.get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
        
        // Find auto transitions from current state
        let auto_transitions: Vec<_> = definition.transitions.iter()
            .filter(|t| t.from == instance.current_state && t.auto)
            .collect();
        
        for transition in auto_transitions {
            if let Some(guard) = &transition.guard {
                let result = self.guard_evaluator
                    .evaluate(guard, instance.subject_id, &instance.subject_type)
                    .await?;
                
                if result.passed {
                    // Execute transition
                    instance = self.execute_transition(instance, &transition.to, None).await?;
                    
                    // Recursively try to advance again
                    return self.try_advance(instance.instance_id).await;
                } else {
                    // Update blockers
                    instance.blockers = result.blockers;
                    self.save_instance(&instance).await?;
                }
            }
        }
        
        Ok(instance)
    }
    
    /// Manually transition to a new state
    pub async fn transition(
        &self,
        instance_id: Uuid,
        to_state: &str,
        by: Option<String>,
        reason: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let instance = self.load_instance(instance_id).await?;
        let definition = self.definitions.get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
        
        // Validate transition exists
        let transition = definition.transitions.iter()
            .find(|t| t.from == instance.current_state && t.to == to_state)
            .ok_or_else(|| WorkflowError::InvalidTransition {
                from: instance.current_state.clone(),
                to: to_state.to_string(),
            })?;
        
        // Check guard if present
        if let Some(guard) = &transition.guard {
            let result = self.guard_evaluator
                .evaluate(guard, instance.subject_id, &instance.subject_type)
                .await?;
            
            if !result.passed {
                return Err(WorkflowError::GuardFailed {
                    guard: guard.clone(),
                    blockers: result.blockers,
                });
            }
        }
        
        self.execute_transition(instance, to_state, by).await
    }
    
    /// Execute a transition
    async fn execute_transition(
        &self,
        mut instance: WorkflowInstance,
        to_state: &str,
        by: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let from_state = instance.current_state.clone();
        
        instance.history.push(StateTransition {
            from_state: from_state.clone(),
            to_state: to_state.to_string(),
            transitioned_at: Utc::now(),
            transitioned_by: by,
            reason: None,
        });
        
        instance.current_state = to_state.to_string();
        instance.state_entered_at = Utc::now();
        instance.updated_at = Utc::now();
        instance.blockers = vec![];  // Clear blockers, will be re-evaluated
        
        self.save_instance(&instance).await?;
        
        // Fire state entry hooks (could emit events, run DSL, etc.)
        self.on_state_entered(&instance, &from_state).await?;
        
        Ok(instance)
    }
    
    /// Evaluate all blockers for current state
    async fn evaluate_blockers(
        &self,
        instance: &WorkflowInstance,
        definition: &WorkflowDefinition,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Find transitions from current state
        let outgoing: Vec<_> = definition.transitions.iter()
            .filter(|t| t.from == instance.current_state)
            .collect();
        
        let mut all_blockers = Vec::new();
        
        for transition in outgoing {
            if let Some(guard) = &transition.guard {
                let result = self.guard_evaluator
                    .evaluate(guard, instance.subject_id, &instance.subject_type)
                    .await?;
                
                if !result.passed {
                    all_blockers.extend(result.blockers);
                }
            }
        }
        
        // Deduplicate blockers
        all_blockers.sort_by(|a, b| a.description.cmp(&b.description));
        all_blockers.dedup_by(|a, b| a.description == b.description);
        
        Ok(all_blockers)
    }
    
    /// Calculate progress percentage
    fn calculate_progress(&self, instance: &WorkflowInstance, definition: &WorkflowDefinition) -> f32 {
        let total_states = definition.states.len() as f32;
        let terminal_states: Vec<_> = definition.states.iter()
            .filter(|(_, s)| s.terminal)
            .collect();
        
        if terminal_states.iter().any(|(name, _)| name.as_str() == instance.current_state) {
            return 100.0;
        }
        
        // Count completed transitions
        let completed = instance.history.len() as f32;
        let estimated_total = total_states - terminal_states.len() as f32;
        
        ((completed / estimated_total) * 100.0).min(99.0)  // Cap at 99% until terminal
    }
    
    async fn on_state_entered(
        &self,
        instance: &WorkflowInstance,
        from_state: &str,
    ) -> Result<(), WorkflowError> {
        // Could trigger:
        // - Event emission
        // - Automatic DSL execution
        // - Notifications
        // - Audit logging
        Ok(())
    }
    
    async fn save_instance(&self, instance: &WorkflowInstance) -> Result<(), WorkflowError> {
        sqlx::query(r#"
            INSERT INTO "ob-poc".workflow_instances 
            (instance_id, workflow_id, version, subject_type, subject_id, 
             current_state, state_entered_at, history, blockers, created_at, updated_at, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (instance_id) DO UPDATE SET
                current_state = $6,
                state_entered_at = $7,
                history = $8,
                blockers = $9,
                updated_at = $11
        "#)
        .bind(instance.instance_id)
        .bind(&instance.workflow_id)
        .bind(instance.version as i32)
        .bind(&instance.subject_type)
        .bind(instance.subject_id)
        .bind(&instance.current_state)
        .bind(instance.state_entered_at)
        .bind(serde_json::to_value(&instance.history).unwrap())
        .bind(serde_json::to_value(&instance.blockers).unwrap())
        .bind(instance.created_at)
        .bind(instance.updated_at)
        .bind(&instance.created_by)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn load_instance(&self, instance_id: Uuid) -> Result<WorkflowInstance, WorkflowError> {
        // Load from DB
        todo!()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStatus {
    pub instance_id: Uuid,
    pub workflow_id: String,
    pub subject_id: Uuid,
    pub current_state: String,
    pub state_description: Option<String>,
    pub is_terminal: bool,
    pub blockers: Vec<Blocker>,
    pub available_transitions: Vec<AvailableTransition>,
    pub available_actions: Vec<AvailableAction>,
    pub progress: f32,
    pub history: Vec<StateTransition>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailableTransition {
    pub to_state: String,
    pub description: String,
    pub is_manual: bool,
    pub guard_status: GuardStatus,
}

#[derive(Debug, Clone, Serialize)]
pub enum GuardStatus {
    Passed,
    Blocked { blockers: Vec<Blocker> },
    NoGuard,
}

#[derive(Debug, Clone, Serialize)]
pub struct AvailableAction {
    pub action: String,
    pub verb: String,
    pub description: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Unknown workflow: {0}")]
    UnknownWorkflow(String),
    
    #[error("No initial state defined")]
    NoInitialState,
    
    #[error("Invalid transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    
    #[error("Guard {guard} failed")]
    GuardFailed { guard: String, blockers: Vec<Blocker> },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

---

## Part 4: MCP Integration

### 4.1 Workflow Tools

```rust
// Add to tools.rs

Tool {
    name: "workflow_status".into(),
    description: "Get current workflow status, blockers, and available actions".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "subject_type": { "type": "string", "enum": ["cbu"] },
            "subject_id": { "type": "string", "format": "uuid" }
        },
        "required": ["subject_type", "subject_id"]
    }),
},

Tool {
    name: "workflow_advance".into(),
    description: "Attempt to advance workflow to next state (evaluates guards)".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "subject_type": { "type": "string" },
            "subject_id": { "type": "string", "format": "uuid" }
        },
        "required": ["subject_type", "subject_id"]
    }),
},

Tool {
    name: "workflow_transition".into(),
    description: "Manually transition to a specific state".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "subject_type": { "type": "string" },
            "subject_id": { "type": "string", "format": "uuid" },
            "to_state": { "type": "string" },
            "reason": { "type": "string" }
        },
        "required": ["subject_type", "subject_id", "to_state"]
    }),
},

Tool {
    name: "resolve_blocker".into(),
    description: "Get DSL template to resolve a specific blocker".into(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "blocker_type": { "type": "string" },
            "context": { "type": "object" }
        },
        "required": ["blocker_type"]
    }),
},
```

### 4.2 Agent Usage Pattern

```
User: "What's blocking the KYC for Apex Fund?"

Agent: [calls workflow_status]
{
  "subject_type": "cbu",
  "subject_id": "uuid-apex-fund"
}

Response:
{
  "current_state": "SCREENING",
  "progress": 45.0,
  "blockers": [
    {
      "blocker_type": { "pending_screening": { "entity_id": "uuid-john" } },
      "description": "Screening required for John Smith",
      "resolution_action": "screening.run"
    },
    {
      "blocker_type": { "unresolved_alert": { "alert_id": "uuid-alert", "entity_id": "uuid-jane" } },
      "description": "Unresolved screening alert for Jane Doe",
      "resolution_action": "screening.clear-alert"
    }
  ],
  "available_actions": [
    { "action": "run_screening", "verb": "screening.run" },
    { "action": "clear_alert", "verb": "screening.clear-alert" }
  ]
}

Agent: "The KYC for Apex Fund is in the SCREENING stage (45% complete). 
        Two items are blocking progress:
        
        1. John Smith needs screening - I can run that now
        2. Jane Doe has an unresolved screening alert that needs review
        
        Would you like me to run the screening for John Smith?"

User: "Yes, and what's the alert about?"

Agent: [calls screening.run for John, fetches alert details]
       "Done. John Smith's screening came back clear.
        
        The alert for Jane Doe is a potential PEP match - she shares a name 
        with a politically exposed person in the UK. This needs manual review.
        Should I mark it as a false positive, or escalate for investigation?"
```

---

## Part 5: Database Schema

```sql
-- migrations/202412_workflow_tables.sql

CREATE TABLE "ob-poc".workflow_instances (
    instance_id UUID PRIMARY KEY,
    workflow_id VARCHAR(100) NOT NULL,
    version INTEGER NOT NULL,
    subject_type VARCHAR(50) NOT NULL,
    subject_id UUID NOT NULL,
    current_state VARCHAR(100) NOT NULL,
    state_entered_at TIMESTAMPTZ NOT NULL,
    history JSONB NOT NULL DEFAULT '[]',
    blockers JSONB NOT NULL DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),
    
    CONSTRAINT uq_workflow_subject UNIQUE (workflow_id, subject_type, subject_id)
);

CREATE INDEX idx_workflow_subject ON "ob-poc".workflow_instances (subject_type, subject_id);
CREATE INDEX idx_workflow_state ON "ob-poc".workflow_instances (workflow_id, current_state);

-- Audit log for all transitions
CREATE TABLE "ob-poc".workflow_audit_log (
    log_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID NOT NULL REFERENCES "ob-poc".workflow_instances(instance_id),
    from_state VARCHAR(100),
    to_state VARCHAR(100) NOT NULL,
    transitioned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    transitioned_by VARCHAR(255),
    reason TEXT,
    blockers_at_transition JSONB
);
```

---

## Part 6: Files to Create

```
rust/src/workflow/
├── mod.rs              # Module exports
├── definition.rs       # WorkflowDefinition, parsing YAML
├── state.rs            # WorkflowInstance, StateTransition, Blocker
├── guards.rs           # GuardEvaluator, guard implementations
├── engine.rs           # WorkflowEngine core logic
└── repository.rs       # Database persistence

rust/config/workflows/
├── kyc_onboarding.yaml
├── ubo_determination.yaml
├── periodic_review.yaml
└── remediation.yaml
```

---

## Implementation Checklist

- [ ] Create database schema for workflow_instances
- [ ] Create `definition.rs` - parse workflow YAML
- [ ] Create `state.rs` - WorkflowInstance, Blocker types
- [ ] Create `guards.rs` - GuardEvaluator with all guards
- [ ] Create `engine.rs` - WorkflowEngine
- [ ] Create `repository.rs` - DB persistence
- [ ] Define `kyc_onboarding.yaml` workflow
- [ ] Define `ubo_determination.yaml` workflow
- [ ] Add MCP tools (workflow_status, workflow_advance, etc.)
- [ ] Wire MCP handlers
- [ ] Test full KYC flow end-to-end
- [ ] Test blocker resolution flow

---

## Key Design Decisions

1. **YAML-defined workflows** - Business can modify without code changes
2. **Guards are code** - Complex logic in Rust, referenced by name from YAML
3. **Blockers are actionable** - Each blocker includes the DSL verb to resolve it
4. **Auto-advance** - Engine automatically transitions when guards pass
5. **Audit trail** - Full history of all transitions with who/when/why
6. **Subject-agnostic** - Works for CBU, Entity, Case, or any future subject type
