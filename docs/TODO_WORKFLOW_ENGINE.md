# TODO: Workflow Orchestration Layer

**Purpose**: Stateful workflow engine for KYC, UBO, and onboarding processes  
**Approach**: Option C - Hybrid (Declarative YAML rules + Custom code guards)  
**Effort**: ~16-20 hours

---

## Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      WORKFLOW AUTHORING                                 │
│                                                                         │
│   YAML Definition ──▶ JSON Schema Validation ──▶ Engine Loads          │
│        │                      │                                         │
│        │              IDE Auto-complete                                 │
│        │              + Error Squiggles                                 │
│        ▼                                                                │
│   CLI Scaffold: cargo xtask workflow new fund_onboarding               │
└─────────────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      WORKFLOW ENGINE                                    │
│                                                                         │
│   Definition ──▶ State Tracker ──▶ Guard Evaluator ──▶ Blocker Builder │
│       │              │                   │                  │           │
│       │              │          ┌────────┴────────┐         │           │
│       │              │          │                 │         │           │
│       │              │    Declarative        Custom         │           │
│       │              │    Rules (YAML)       Guards         │           │
│       │              │                       (Code)         │           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Part 1: Directory Structure

```
rust/
├── config/
│   └── workflows/
│       ├── schema/
│       │   └── workflow.schema.json      # JSON Schema for validation
│       ├── kyc_onboarding.yaml           # KYC workflow
│       ├── fund_onboarding.yaml          # Fund-specific workflow
│       ├── corporate_onboarding.yaml     # Corporate workflow
│       ├── ubo_determination.yaml        # UBO sub-workflow
│       └── periodic_review.yaml          # Periodic review workflow
│
├── src/
│   └── workflow/
│       ├── mod.rs                        # Module exports
│       ├── definition.rs                 # WorkflowDefinition, State, Transition
│       ├── instance.rs                   # WorkflowInstance, StateTransition
│       ├── blocker.rs                    # Blocker, BlockerType
│       ├── engine.rs                     # WorkflowEngine
│       ├── loader.rs                     # Load & validate YAML definitions
│       ├── repository.rs                 # Database persistence
│       ├── rules/
│       │   ├── mod.rs                    # Rule trait, RuleEvaluator
│       │   ├── role_count.rs             # RoleCountRule
│       │   ├── ownership.rs              # OwnershipCompleteRule
│       │   ├── screening.rs              # ScreeningRules
│       │   ├── documents.rs              # DocumentRules
│       │   └── case_status.rs            # CaseStatusRule
│       └── guards/
│           ├── mod.rs                    # Guard trait, GuardRegistry
│           ├── fund.rs                   # Fund-specific guards
│           ├── corporate.rs              # Corporate-specific guards
│           └── ubo.rs                    # UBO-specific guards
│
└── xtask/
    └── src/
        └── workflow.rs                   # CLI: new, validate, visualize
```

---

## Part 2: JSON Schema for YAML Validation

Create schema so IDEs provide auto-completion and validation:

```json
// rust/config/workflows/schema/workflow.schema.json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "https://ob-poc.internal/workflow.schema.json",
  "title": "Workflow Definition",
  "description": "Defines a state machine workflow for KYC/onboarding processes",
  "type": "object",
  "required": ["workflow", "version", "states", "transitions"],
  "properties": {
    "workflow": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9_]*$",
      "description": "Unique workflow identifier (snake_case)"
    },
    "version": {
      "type": "integer",
      "minimum": 1,
      "description": "Workflow version for migration support"
    },
    "description": {
      "type": "string",
      "description": "Human-readable description"
    },
    "trigger": {
      "$ref": "#/definitions/trigger"
    },
    "states": {
      "type": "object",
      "additionalProperties": {
        "$ref": "#/definitions/state"
      },
      "description": "Map of state names to state definitions"
    },
    "transitions": {
      "type": "array",
      "items": {
        "$ref": "#/definitions/transition"
      },
      "description": "Valid state transitions"
    },
    "actions": {
      "type": "object",
      "additionalProperties": {
        "type": "array",
        "items": {
          "$ref": "#/definitions/action"
        }
      },
      "description": "Available actions per state"
    }
  },
  "definitions": {
    "trigger": {
      "type": "object",
      "required": ["on"],
      "properties": {
        "on": {
          "type": "string",
          "description": "Event that triggers workflow (e.g., cbu.created)"
        },
        "when": {
          "type": "object",
          "additionalProperties": true,
          "description": "Conditions for trigger (field: value)"
        }
      }
    },
    "state": {
      "type": "object",
      "properties": {
        "description": {
          "type": "string"
        },
        "initial": {
          "type": "boolean",
          "default": false
        },
        "terminal": {
          "type": "boolean",
          "default": false
        }
      }
    },
    "transition": {
      "type": "object",
      "required": ["from", "to"],
      "properties": {
        "from": {
          "type": "string",
          "description": "Source state"
        },
        "to": {
          "type": "string",
          "description": "Target state"
        },
        "auto": {
          "type": "boolean",
          "default": false,
          "description": "Auto-transition when guard passes"
        },
        "manual": {
          "type": "boolean",
          "default": false,
          "description": "Requires explicit user action"
        },
        "guard": {
          "$ref": "#/definitions/guard"
        }
      }
    },
    "guard": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "rules": {
              "type": "array",
              "items": {
                "$ref": "#/definitions/rule"
              }
            },
            "custom": {
              "type": "string",
              "description": "Name of custom guard function"
            }
          }
        },
        {
          "type": "object",
          "properties": {
            "all": {
              "type": "array",
              "items": {
                "$ref": "#/definitions/rule"
              },
              "description": "All rules must pass"
            }
          }
        },
        {
          "type": "object",
          "properties": {
            "any": {
              "type": "array",
              "items": {
                "$ref": "#/definitions/rule"
              },
              "description": "At least one rule must pass"
            }
          }
        }
      ]
    },
    "rule": {
      "type": "object",
      "minProperties": 1,
      "maxProperties": 1,
      "properties": {
        "role_count": {
          "type": "object",
          "required": ["role", "min"],
          "properties": {
            "role": { "type": "string" },
            "min": { "type": "integer", "minimum": 0 },
            "max": { "type": "integer", "minimum": 0 },
            "if": { "type": "object" }
          }
        },
        "ownership_complete": {
          "type": "number",
          "minimum": 0,
          "maximum": 100
        },
        "all_participants_screened": {
          "type": "boolean"
        },
        "no_unresolved_alerts": {
          "type": "boolean"
        },
        "all_ubos_verified": {
          "type": "boolean"
        },
        "documents_present": {
          "type": "array",
          "items": { "type": "string" }
        },
        "per_role_documents": {
          "type": "object",
          "required": ["role", "documents"],
          "properties": {
            "role": { "type": "string" },
            "documents": {
              "type": "array",
              "items": { "type": "string" }
            }
          }
        },
        "case_status": {
          "type": "string"
        },
        "field_equals": {
          "type": "object",
          "required": ["field", "value"],
          "properties": {
            "field": { "type": "string" },
            "value": {}
          }
        },
        "custom": {
          "type": "string",
          "description": "Name of custom guard"
        }
      }
    },
    "action": {
      "type": "object",
      "required": ["verb"],
      "properties": {
        "verb": {
          "type": "string",
          "description": "DSL verb (e.g., cbu.assign-role)"
        },
        "description": {
          "type": "string"
        },
        "params": {
          "type": "object",
          "description": "Pre-filled parameters"
        }
      }
    }
  }
}
```

**IDE Setup** - Add to workspace settings:

```json
// .vscode/settings.json
{
  "yaml.schemas": {
    "./rust/config/workflows/schema/workflow.schema.json": "rust/config/workflows/*.yaml"
  }
}
```

---

## Part 3: Core Types

### 3.1 `rust/src/workflow/definition.rs`

```rust
//! Workflow definition types - parsed from YAML

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowDefinition {
    pub workflow: String,
    pub version: u32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub trigger: Option<WorkflowTrigger>,
    pub states: HashMap<String, StateDefinition>,
    pub transitions: Vec<TransitionDefinition>,
    #[serde(default)]
    pub actions: HashMap<String, Vec<ActionDefinition>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorkflowTrigger {
    pub on: String,  // e.g., "cbu.created"
    #[serde(default)]
    pub when: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateDefinition {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub initial: bool,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransitionDefinition {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub guard: Option<GuardDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GuardDefinition {
    /// All rules must pass
    All { all: Vec<RuleDefinition> },
    /// At least one rule must pass
    Any { any: Vec<RuleDefinition> },
    /// Rules + optional custom guard
    RulesAndCustom {
        #[serde(default)]
        rules: Vec<RuleDefinition>,
        #[serde(default)]
        custom: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleDefinition {
    RoleCount {
        role: String,
        min: u32,
        #[serde(default)]
        max: Option<u32>,
        #[serde(rename = "if", default)]
        condition: Option<HashMap<String, serde_json::Value>>,
    },
    OwnershipComplete(f64),
    AllParticipantsScreened(bool),
    NoUnresolvedAlerts(bool),
    AllUbosVerified(bool),
    DocumentsPresent(Vec<String>),
    PerRoleDocuments {
        role: String,
        documents: Vec<String>,
    },
    CaseStatus(String),
    FieldEquals {
        field: String,
        value: serde_json::Value,
    },
    Custom(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionDefinition {
    pub verb: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

impl WorkflowDefinition {
    /// Get the initial state name
    pub fn initial_state(&self) -> Option<&str> {
        self.states.iter()
            .find(|(_, s)| s.initial)
            .map(|(name, _)| name.as_str())
    }
    
    /// Get terminal state names
    pub fn terminal_states(&self) -> Vec<&str> {
        self.states.iter()
            .filter(|(_, s)| s.terminal)
            .map(|(name, _)| name.as_str())
            .collect()
    }
    
    /// Get transitions from a given state
    pub fn transitions_from(&self, state: &str) -> Vec<&TransitionDefinition> {
        self.transitions.iter()
            .filter(|t| t.from == state)
            .collect()
    }
    
    /// Get available actions for a state
    pub fn actions_for(&self, state: &str) -> &[ActionDefinition] {
        self.actions.get(state)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    
    /// Validate definition consistency
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Must have exactly one initial state
        let initial_count = self.states.values().filter(|s| s.initial).count();
        if initial_count != 1 {
            return Err(ValidationError::InvalidInitialState(initial_count));
        }
        
        // Must have at least one terminal state
        if self.terminal_states().is_empty() {
            return Err(ValidationError::NoTerminalState);
        }
        
        // All transitions must reference valid states
        for t in &self.transitions {
            if !self.states.contains_key(&t.from) {
                return Err(ValidationError::UnknownState(t.from.clone()));
            }
            if !self.states.contains_key(&t.to) {
                return Err(ValidationError::UnknownState(t.to.clone()));
            }
        }
        
        // All action states must exist
        for state in self.actions.keys() {
            if !self.states.contains_key(state) {
                return Err(ValidationError::UnknownState(state.clone()));
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Expected exactly 1 initial state, found {0}")]
    InvalidInitialState(usize),
    #[error("No terminal state defined")]
    NoTerminalState,
    #[error("Unknown state referenced: {0}")]
    UnknownState(String),
    #[error("Unknown verb in actions: {0}")]
    UnknownVerb(String),
    #[error("Unknown custom guard: {0}")]
    UnknownGuard(String),
}
```

### 3.2 `rust/src/workflow/instance.rs`

```rust
//! Workflow instance - a running workflow

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::blocker::Blocker;

/// A running instance of a workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    pub instance_id: Uuid,
    pub workflow_id: String,
    pub version: u32,
    
    /// What entity this workflow is for
    pub subject_type: String,
    pub subject_id: Uuid,
    
    /// Current state
    pub current_state: String,
    pub state_entered_at: DateTime<Utc>,
    
    /// State history
    pub history: Vec<StateTransition>,
    
    /// Current blockers (cached, re-evaluated on query)
    pub blockers: Vec<Blocker>,
    
    /// Metadata
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

impl WorkflowInstance {
    pub fn new(
        workflow_id: String,
        version: u32,
        subject_type: String,
        subject_id: Uuid,
        initial_state: String,
        created_by: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            instance_id: Uuid::new_v4(),
            workflow_id,
            version,
            subject_type,
            subject_id,
            current_state: initial_state,
            state_entered_at: now,
            history: vec![],
            blockers: vec![],
            created_at: now,
            updated_at: now,
            created_by,
        }
    }
    
    pub fn transition(&mut self, to_state: String, by: Option<String>, reason: Option<String>) {
        self.history.push(StateTransition {
            from_state: self.current_state.clone(),
            to_state: to_state.clone(),
            transitioned_at: Utc::now(),
            transitioned_by: by,
            reason,
        });
        
        self.current_state = to_state;
        self.state_entered_at = Utc::now();
        self.updated_at = Utc::now();
        self.blockers.clear();
    }
    
    pub fn time_in_current_state(&self) -> chrono::Duration {
        Utc::now() - self.state_entered_at
    }
}
```

### 3.3 `rust/src/workflow/blocker.rs`

```rust
//! Blocker types - what's preventing workflow advancement

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A blocker preventing workflow advancement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    pub blocker_type: BlockerType,
    pub description: String,
    /// DSL verb that can resolve this blocker
    pub resolution_action: Option<String>,
    /// Additional context for resolution
    pub resolution_context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockerType {
    MissingRole {
        role: String,
        required: u32,
        current: u32,
    },
    MissingDocument {
        document_type: String,
        for_entity: Option<Uuid>,
        for_entity_name: Option<String>,
    },
    PendingScreening {
        entity_id: Uuid,
        entity_name: String,
    },
    UnresolvedAlert {
        alert_id: Uuid,
        entity_id: Uuid,
        entity_name: String,
        alert_type: String,
    },
    IncompleteOwnership {
        current_total: f64,
        required: f64,
    },
    UnverifiedUbo {
        ubo_id: Uuid,
        person_id: Uuid,
        person_name: String,
    },
    CaseNotInStatus {
        current_status: Option<String>,
        required_status: String,
    },
    ManualApprovalRequired,
    CustomGuardFailed {
        guard_name: String,
        message: String,
    },
}

impl Blocker {
    pub fn missing_role(role: &str, required: u32, current: u32) -> Self {
        Self {
            blocker_type: BlockerType::MissingRole {
                role: role.to_string(),
                required,
                current,
            },
            description: format!(
                "Need {} {} (have {})",
                required,
                role.to_lowercase().replace('_', " "),
                current
            ),
            resolution_action: Some("cbu.assign-role".to_string()),
            resolution_context: [
                ("role".to_string(), serde_json::json!(role)),
            ].into(),
        }
    }
    
    pub fn missing_document(doc_type: &str, for_entity: Option<Uuid>, entity_name: Option<&str>) -> Self {
        let desc = match entity_name {
            Some(name) => format!("{} required for {}", doc_type.replace('_', " "), name),
            None => format!("{} required", doc_type.replace('_', " ")),
        };
        
        Self {
            blocker_type: BlockerType::MissingDocument {
                document_type: doc_type.to_string(),
                for_entity,
                for_entity_name: entity_name.map(String::from),
            },
            description: desc,
            resolution_action: Some("document.upload".to_string()),
            resolution_context: [
                ("document_type".to_string(), serde_json::json!(doc_type)),
            ].into(),
        }
    }
    
    pub fn pending_screening(entity_id: Uuid, entity_name: &str) -> Self {
        Self {
            blocker_type: BlockerType::PendingScreening {
                entity_id,
                entity_name: entity_name.to_string(),
            },
            description: format!("Screening required for {}", entity_name),
            resolution_action: Some("screening.run".to_string()),
            resolution_context: [
                ("entity_id".to_string(), serde_json::json!(entity_id)),
            ].into(),
        }
    }
    
    pub fn unresolved_alert(alert_id: Uuid, entity_id: Uuid, entity_name: &str, alert_type: &str) -> Self {
        Self {
            blocker_type: BlockerType::UnresolvedAlert {
                alert_id,
                entity_id,
                entity_name: entity_name.to_string(),
                alert_type: alert_type.to_string(),
            },
            description: format!("Unresolved {} alert for {}", alert_type, entity_name),
            resolution_action: Some("screening.clear-alert".to_string()),
            resolution_context: [
                ("alert_id".to_string(), serde_json::json!(alert_id)),
            ].into(),
        }
    }
    
    pub fn incomplete_ownership(current: f64, required: f64) -> Self {
        Self {
            blocker_type: BlockerType::IncompleteOwnership {
                current_total: current,
                required,
            },
            description: format!("Ownership {:.1}% of {:.0}% documented", current, required),
            resolution_action: Some("ubo.add-ownership".to_string()),
            resolution_context: Default::default(),
        }
    }
    
    pub fn unverified_ubo(ubo_id: Uuid, person_id: Uuid, person_name: &str) -> Self {
        Self {
            blocker_type: BlockerType::UnverifiedUbo {
                ubo_id,
                person_id,
                person_name: person_name.to_string(),
            },
            description: format!("UBO verification required for {}", person_name),
            resolution_action: Some("ubo.verify-ubo".to_string()),
            resolution_context: [
                ("ubo_id".to_string(), serde_json::json!(ubo_id)),
            ].into(),
        }
    }
}
```

---

## Part 4: Rule Evaluator

### 4.1 `rust/src/workflow/rules/mod.rs`

```rust
//! Declarative rule evaluation
//!
//! Rules are defined in YAML, evaluated by generic code.

mod role_count;
mod ownership;
mod screening;
mod documents;
mod case_status;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use super::blocker::Blocker;
use super::definition::RuleDefinition;

pub use role_count::RoleCountRule;
pub use ownership::OwnershipRule;
pub use screening::{AllScreenedRule, NoAlertsRule};
pub use documents::{DocumentsPresentRule, PerRoleDocumentsRule};
pub use case_status::CaseStatusRule;

/// Result of evaluating a rule
#[derive(Debug)]
pub struct RuleResult {
    pub passed: bool,
    pub blockers: Vec<Blocker>,
}

impl RuleResult {
    pub fn passed() -> Self {
        Self { passed: true, blockers: vec![] }
    }
    
    pub fn failed(blockers: Vec<Blocker>) -> Self {
        Self { passed: false, blockers }
    }
    
    pub fn single_blocker(blocker: Blocker) -> Self {
        Self { passed: false, blockers: vec![blocker] }
    }
}

/// Context for rule evaluation
pub struct RuleContext<'a> {
    pub pool: &'a PgPool,
    pub subject_type: &'a str,
    pub subject_id: Uuid,
    /// Additional context (e.g., fund_type for conditional rules)
    pub attributes: std::collections::HashMap<String, serde_json::Value>,
}

/// Evaluates declarative rules from YAML
pub struct RuleEvaluator {
    pool: PgPool,
}

impl RuleEvaluator {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Evaluate a single rule
    pub async fn evaluate(
        &self,
        rule: &RuleDefinition,
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<RuleResult, sqlx::Error> {
        let ctx = RuleContext {
            pool: &self.pool,
            subject_type,
            subject_id,
            attributes: Default::default(),
        };
        
        match rule {
            RuleDefinition::RoleCount { role, min, max, condition } => {
                // Check condition first if present
                if let Some(cond) = condition {
                    if !self.check_condition(&ctx, cond).await? {
                        return Ok(RuleResult::passed()); // Rule doesn't apply
                    }
                }
                RoleCountRule::evaluate(&ctx, role, *min, *max).await
            }
            RuleDefinition::OwnershipComplete(threshold) => {
                OwnershipRule::evaluate(&ctx, *threshold).await
            }
            RuleDefinition::AllParticipantsScreened(required) => {
                if *required {
                    AllScreenedRule::evaluate(&ctx).await
                } else {
                    Ok(RuleResult::passed())
                }
            }
            RuleDefinition::NoUnresolvedAlerts(required) => {
                if *required {
                    NoAlertsRule::evaluate(&ctx).await
                } else {
                    Ok(RuleResult::passed())
                }
            }
            RuleDefinition::AllUbosVerified(required) => {
                if *required {
                    ownership::AllUbosVerifiedRule::evaluate(&ctx).await
                } else {
                    Ok(RuleResult::passed())
                }
            }
            RuleDefinition::DocumentsPresent(doc_types) => {
                DocumentsPresentRule::evaluate(&ctx, doc_types).await
            }
            RuleDefinition::PerRoleDocuments { role, documents } => {
                PerRoleDocumentsRule::evaluate(&ctx, role, documents).await
            }
            RuleDefinition::CaseStatus(status) => {
                CaseStatusRule::evaluate(&ctx, status).await
            }
            RuleDefinition::FieldEquals { field, value } => {
                self.evaluate_field_equals(&ctx, field, value).await
            }
            RuleDefinition::Custom(guard_name) => {
                // Delegate to custom guard - handled by engine
                Ok(RuleResult::passed())
            }
        }
    }
    
    /// Evaluate all rules (AND logic)
    pub async fn evaluate_all(
        &self,
        rules: &[RuleDefinition],
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<RuleResult, sqlx::Error> {
        let mut all_blockers = Vec::new();
        
        for rule in rules {
            let result = self.evaluate(rule, subject_id, subject_type).await?;
            if !result.passed {
                all_blockers.extend(result.blockers);
            }
        }
        
        if all_blockers.is_empty() {
            Ok(RuleResult::passed())
        } else {
            Ok(RuleResult::failed(all_blockers))
        }
    }
    
    /// Evaluate any rules (OR logic)
    pub async fn evaluate_any(
        &self,
        rules: &[RuleDefinition],
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<RuleResult, sqlx::Error> {
        for rule in rules {
            let result = self.evaluate(rule, subject_id, subject_type).await?;
            if result.passed {
                return Ok(RuleResult::passed());
            }
        }
        
        // All failed - return blockers from all
        let mut all_blockers = Vec::new();
        for rule in rules {
            let result = self.evaluate(rule, subject_id, subject_type).await?;
            all_blockers.extend(result.blockers);
        }
        
        Ok(RuleResult::failed(all_blockers))
    }
    
    async fn check_condition(
        &self,
        ctx: &RuleContext<'_>,
        condition: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<bool, sqlx::Error> {
        // Query subject to check field values
        // e.g., { "fund_type": "UNIT_TRUST" }
        for (field, expected) in condition {
            let actual: Option<serde_json::Value> = sqlx::query_scalar(&format!(
                r#"SELECT {}::text FROM "ob-poc".cbus WHERE cbu_id = $1"#,
                field
            ))
            .bind(ctx.subject_id)
            .fetch_optional(ctx.pool)
            .await?;
            
            if actual.as_ref() != Some(expected) {
                return Ok(false);
            }
        }
        Ok(true)
    }
    
    async fn evaluate_field_equals(
        &self,
        ctx: &RuleContext<'_>,
        field: &str,
        expected: &serde_json::Value,
    ) -> Result<RuleResult, sqlx::Error> {
        let actual: Option<String> = sqlx::query_scalar(&format!(
            r#"SELECT {}::text FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            field
        ))
        .bind(ctx.subject_id)
        .fetch_optional(ctx.pool)
        .await?;
        
        let matches = actual.as_ref().map(|v| serde_json::json!(v) == *expected).unwrap_or(false);
        
        if matches {
            Ok(RuleResult::passed())
        } else {
            Ok(RuleResult::single_blocker(Blocker {
                blocker_type: super::blocker::BlockerType::CustomGuardFailed {
                    guard_name: "field_equals".to_string(),
                    message: format!("{} must equal {:?}", field, expected),
                },
                description: format!("{} must equal {:?}", field, expected),
                resolution_action: None,
                resolution_context: Default::default(),
            }))
        }
    }
}
```

### 4.2 `rust/src/workflow/rules/role_count.rs`

```rust
use sqlx::PgPool;
use uuid::Uuid;

use super::{RuleContext, RuleResult};
use crate::workflow::blocker::Blocker;

pub struct RoleCountRule;

impl RoleCountRule {
    pub async fn evaluate(
        ctx: &RuleContext<'_>,
        role: &str,
        min: u32,
        max: Option<u32>,
    ) -> Result<RuleResult, sqlx::Error> {
        let count: i64 = sqlx::query_scalar(r#"
            SELECT COUNT(*) FROM "ob-poc".cbu_roles cr
            JOIN "ob-poc".roles r ON cr.role_id = r.role_id
            WHERE cr.cbu_id = $1 
            AND r.role_code = $2
            AND cr.effective_to IS NULL
        "#)
        .bind(ctx.subject_id)
        .bind(role)
        .fetch_one(ctx.pool)
        .await?;
        
        let count = count as u32;
        
        if count < min {
            return Ok(RuleResult::single_blocker(
                Blocker::missing_role(role, min, count)
            ));
        }
        
        if let Some(max) = max {
            if count > max {
                return Ok(RuleResult::single_blocker(Blocker {
                    blocker_type: crate::workflow::blocker::BlockerType::CustomGuardFailed {
                        guard_name: "role_count_max".to_string(),
                        message: format!("Too many {} (max: {}, have: {})", role, max, count),
                    },
                    description: format!("Maximum {} {} allowed (have {})", max, role, count),
                    resolution_action: Some("cbu.remove-role".to_string()),
                    resolution_context: Default::default(),
                }));
            }
        }
        
        Ok(RuleResult::passed())
    }
}
```

---

## Part 5: Custom Guard Registry

### 5.1 `rust/src/workflow/guards/mod.rs`

```rust
//! Custom guards for complex logic that can't be expressed declaratively

mod fund;
mod corporate;
mod ubo;

use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use super::blocker::Blocker;

/// Result of guard evaluation
#[derive(Debug)]
pub struct GuardResult {
    pub passed: bool,
    pub blockers: Vec<Blocker>,
}

impl GuardResult {
    pub fn passed() -> Self {
        Self { passed: true, blockers: vec![] }
    }
    
    pub fn failed(blockers: Vec<Blocker>) -> Self {
        Self { passed: false, blockers }
    }
}

/// Context for guard evaluation
pub struct GuardContext<'a> {
    pub pool: &'a PgPool,
    pub subject_type: &'a str,
    pub subject_id: Uuid,
}

/// Trait for custom guards
#[async_trait]
pub trait Guard: Send + Sync {
    /// Evaluate the guard
    async fn evaluate(&self, ctx: &GuardContext<'_>) -> Result<GuardResult, sqlx::Error>;
    
    /// Human-readable description
    fn description(&self) -> &str;
}

/// Registry of custom guards
pub struct GuardRegistry {
    guards: HashMap<String, Box<dyn Guard>>,
}

impl GuardRegistry {
    pub fn new() -> Self {
        let mut registry = Self { guards: HashMap::new() };
        
        // Register built-in custom guards
        registry.register("fund_structure_valid", fund::FundStructureValidGuard);
        registry.register("management_company_linked", fund::ManagementCompanyLinkedGuard);
        registry.register("umbrella_linked_if_subfund", fund::UmbrellaLinkedIfSubfundGuard);
        registry.register("corporate_structure_valid", corporate::CorporateStructureValidGuard);
        registry.register("no_circular_ownership", ubo::NoCircularOwnershipGuard);
        registry.register("ubo_chain_complete", ubo::UboChainCompleteGuard);
        
        registry
    }
    
    pub fn register<G: Guard + 'static>(&mut self, name: &str, guard: G) {
        self.guards.insert(name.to_string(), Box::new(guard));
    }
    
    pub fn get(&self, name: &str) -> Option<&dyn Guard> {
        self.guards.get(name).map(|b| b.as_ref())
    }
    
    pub fn list(&self) -> Vec<(&str, &str)> {
        self.guards.iter()
            .map(|(name, guard)| (name.as_str(), guard.description()))
            .collect()
    }
}

impl Default for GuardRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

### 5.2 `rust/src/workflow/guards/fund.rs`

```rust
use async_trait::async_trait;
use super::{Guard, GuardContext, GuardResult};
use crate::workflow::blocker::{Blocker, BlockerType};

pub struct FundStructureValidGuard;

#[async_trait]
impl Guard for FundStructureValidGuard {
    async fn evaluate(&self, ctx: &GuardContext<'_>) -> Result<GuardResult, sqlx::Error> {
        // Complex fund structure validation:
        // - If sub-fund, must be linked to umbrella
        // - Must have management company
        // - Jurisdiction must be valid for fund type
        
        // Check for management company
        let has_mgmt_co: bool = sqlx::query_scalar(r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".fund_relationships fr
                WHERE fr.fund_cbu_id = $1
                AND fr.relationship_type = 'MANAGEMENT_COMPANY'
            )
        "#)
        .bind(ctx.subject_id)
        .fetch_one(ctx.pool)
        .await?;
        
        if !has_mgmt_co {
            return Ok(GuardResult::failed(vec![Blocker {
                blocker_type: BlockerType::CustomGuardFailed {
                    guard_name: "fund_structure_valid".to_string(),
                    message: "Fund must have a management company".to_string(),
                },
                description: "Management company required".to_string(),
                resolution_action: Some("fund.link-management-company".to_string()),
                resolution_context: Default::default(),
            }]));
        }
        
        Ok(GuardResult::passed())
    }
    
    fn description(&self) -> &str {
        "Validates fund structure (management company, umbrella links, jurisdiction)"
    }
}

pub struct ManagementCompanyLinkedGuard;

#[async_trait]
impl Guard for ManagementCompanyLinkedGuard {
    async fn evaluate(&self, ctx: &GuardContext<'_>) -> Result<GuardResult, sqlx::Error> {
        let has_mgmt_co: bool = sqlx::query_scalar(r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".fund_relationships fr
                WHERE fr.fund_cbu_id = $1
                AND fr.relationship_type = 'MANAGEMENT_COMPANY'
            )
        "#)
        .bind(ctx.subject_id)
        .fetch_one(ctx.pool)
        .await?;
        
        if has_mgmt_co {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::failed(vec![Blocker {
                blocker_type: BlockerType::CustomGuardFailed {
                    guard_name: "management_company_linked".to_string(),
                    message: "Management company not linked".to_string(),
                },
                description: "Link a management company".to_string(),
                resolution_action: Some("fund.link-management-company".to_string()),
                resolution_context: Default::default(),
            }]))
        }
    }
    
    fn description(&self) -> &str {
        "Checks that fund has a linked management company"
    }
}

pub struct UmbrellaLinkedIfSubfundGuard;

#[async_trait]
impl Guard for UmbrellaLinkedIfSubfundGuard {
    async fn evaluate(&self, ctx: &GuardContext<'_>) -> Result<GuardResult, sqlx::Error> {
        // Check if this is a sub-fund
        let is_subfund: bool = sqlx::query_scalar(r#"
            SELECT cbu_type = 'SUB_FUND' FROM "ob-poc".cbus WHERE cbu_id = $1
        "#)
        .bind(ctx.subject_id)
        .fetch_one(ctx.pool)
        .await?;
        
        if !is_subfund {
            return Ok(GuardResult::passed()); // Not a sub-fund, rule doesn't apply
        }
        
        // Check umbrella link
        let has_umbrella: bool = sqlx::query_scalar(r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".fund_relationships fr
                WHERE fr.fund_cbu_id = $1
                AND fr.relationship_type = 'UMBRELLA'
            )
        "#)
        .bind(ctx.subject_id)
        .fetch_one(ctx.pool)
        .await?;
        
        if has_umbrella {
            Ok(GuardResult::passed())
        } else {
            Ok(GuardResult::failed(vec![Blocker {
                blocker_type: BlockerType::CustomGuardFailed {
                    guard_name: "umbrella_linked_if_subfund".to_string(),
                    message: "Sub-fund must be linked to umbrella fund".to_string(),
                },
                description: "Link to umbrella fund".to_string(),
                resolution_action: Some("fund.link-umbrella".to_string()),
                resolution_context: Default::default(),
            }]))
        }
    }
    
    fn description(&self) -> &str {
        "Ensures sub-funds are linked to their umbrella fund"
    }
}
```

---

## Part 6: Workflow Engine

### 6.1 `rust/src/workflow/engine.rs`

```rust
//! Core workflow engine

use std::collections::HashMap;
use std::sync::Arc;
use sqlx::PgPool;
use uuid::Uuid;

use super::definition::{WorkflowDefinition, GuardDefinition, RuleDefinition};
use super::instance::WorkflowInstance;
use super::blocker::Blocker;
use super::rules::RuleEvaluator;
use super::guards::{GuardRegistry, GuardContext};

pub struct WorkflowEngine {
    pool: PgPool,
    definitions: HashMap<String, WorkflowDefinition>,
    rule_evaluator: RuleEvaluator,
    guard_registry: Arc<GuardRegistry>,
}

impl WorkflowEngine {
    pub fn new(
        pool: PgPool,
        definitions: HashMap<String, WorkflowDefinition>,
        guard_registry: Arc<GuardRegistry>,
    ) -> Self {
        Self {
            rule_evaluator: RuleEvaluator::new(pool.clone()),
            pool,
            definitions,
            guard_registry,
        }
    }
    
    /// Start a new workflow instance
    pub async fn start(
        &self,
        workflow_id: &str,
        subject_type: &str,
        subject_id: Uuid,
        created_by: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let def = self.definitions.get(workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(workflow_id.to_string()))?;
        
        let initial_state = def.initial_state()
            .ok_or(WorkflowError::NoInitialState)?;
        
        let instance = WorkflowInstance::new(
            workflow_id.to_string(),
            def.version,
            subject_type.to_string(),
            subject_id,
            initial_state.to_string(),
            created_by,
        );
        
        self.save_instance(&instance).await?;
        
        // Try to auto-advance from initial state
        self.try_advance(instance.instance_id).await
    }
    
    /// Get current workflow status with blockers
    pub async fn status(&self, instance_id: Uuid) -> Result<WorkflowStatus, WorkflowError> {
        let instance = self.load_instance(instance_id).await?;
        let def = self.definitions.get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
        
        // Evaluate blockers for all outgoing transitions
        let blockers = self.evaluate_all_blockers(&instance, def).await?;
        
        // Get available actions
        let actions: Vec<_> = def.actions_for(&instance.current_state)
            .iter()
            .map(|a| AvailableAction {
                verb: a.verb.clone(),
                description: a.description.clone(),
            })
            .collect();
        
        // Get available transitions
        let transitions = self.get_available_transitions(&instance, def).await?;
        
        Ok(WorkflowStatus {
            instance_id: instance.instance_id,
            workflow_id: instance.workflow_id.clone(),
            subject_type: instance.subject_type.clone(),
            subject_id: instance.subject_id,
            current_state: instance.current_state.clone(),
            state_description: def.states.get(&instance.current_state)
                .and_then(|s| s.description.clone()),
            is_terminal: def.states.get(&instance.current_state)
                .map(|s| s.terminal)
                .unwrap_or(false),
            blockers,
            available_actions: actions,
            available_transitions: transitions,
            progress: self.calculate_progress(&instance, def),
            history: instance.history.clone(),
        })
    }
    
    /// Try to automatically advance the workflow
    pub async fn try_advance(&self, instance_id: Uuid) -> Result<WorkflowInstance, WorkflowError> {
        let mut instance = self.load_instance(instance_id).await?;
        let def = self.definitions.get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
        
        // Find auto transitions from current state
        for transition in def.transitions_from(&instance.current_state) {
            if !transition.auto {
                continue;
            }
            
            // Evaluate guard
            let result = self.evaluate_guard(
                transition.guard.as_ref(),
                instance.subject_id,
                &instance.subject_type,
            ).await?;
            
            if result.passed {
                // Execute transition
                instance.transition(transition.to.clone(), None, Some("Auto-advanced".to_string()));
                self.save_instance(&instance).await?;
                
                // Recursively try to advance again
                return self.try_advance(instance.instance_id).await;
            }
        }
        
        // Update blockers
        instance.blockers = self.evaluate_all_blockers(&instance, def).await?;
        self.save_instance(&instance).await?;
        
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
        let mut instance = self.load_instance(instance_id).await?;
        let def = self.definitions.get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;
        
        // Find the transition
        let transition = def.transitions_from(&instance.current_state)
            .into_iter()
            .find(|t| t.to == to_state)
            .ok_or_else(|| WorkflowError::InvalidTransition {
                from: instance.current_state.clone(),
                to: to_state.to_string(),
            })?;
        
        // Evaluate guard if present
        if let Some(guard) = &transition.guard {
            let result = self.evaluate_guard(
                Some(guard),
                instance.subject_id,
                &instance.subject_type,
            ).await?;
            
            if !result.passed {
                return Err(WorkflowError::GuardFailed {
                    blockers: result.blockers,
                });
            }
        }
        
        // Execute transition
        instance.transition(to_state.to_string(), by, reason);
        self.save_instance(&instance).await?;
        
        // Try to auto-advance from new state
        self.try_advance(instance.instance_id).await
    }
    
    /// Evaluate a guard definition
    async fn evaluate_guard(
        &self,
        guard: Option<&GuardDefinition>,
        subject_id: Uuid,
        subject_type: &str,
    ) -> Result<GuardEvalResult, WorkflowError> {
        let guard = match guard {
            None => return Ok(GuardEvalResult::passed()),
            Some(g) => g,
        };
        
        match guard {
            GuardDefinition::All { all } => {
                let result = self.rule_evaluator.evaluate_all(all, subject_id, subject_type).await?;
                Ok(GuardEvalResult {
                    passed: result.passed,
                    blockers: result.blockers,
                })
            }
            GuardDefinition::Any { any } => {
                let result = self.rule_evaluator.evaluate_any(any, subject_id, subject_type).await?;
                Ok(GuardEvalResult {
                    passed: result.passed,
                    blockers: result.blockers,
                })
            }
            GuardDefinition::RulesAndCustom { rules, custom } => {
                // Evaluate declarative rules
                let mut all_blockers = Vec::new();
                
                if !rules.is_empty() {
                    let result = self.rule_evaluator.evaluate_all(rules, subject_id, subject_type).await?;
                    if !result.passed {
                        all_blockers.extend(result.blockers);
                    }
                }
                
                // Evaluate custom guard if present
                if let Some(guard_name) = custom {
                    if let Some(guard) = self.guard_registry.get(guard_name) {
                        let ctx = GuardContext {
                            pool: &self.pool,
                            subject_type,
                            subject_id,
                        };
                        let result = guard.evaluate(&ctx).await?;
                        if !result.passed {
                            all_blockers.extend(result.blockers);
                        }
                    }
                }
                
                Ok(GuardEvalResult {
                    passed: all_blockers.is_empty(),
                    blockers: all_blockers,
                })
            }
        }
    }
    
    async fn evaluate_all_blockers(
        &self,
        instance: &WorkflowInstance,
        def: &WorkflowDefinition,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        let mut all_blockers = Vec::new();
        
        for transition in def.transitions_from(&instance.current_state) {
            let result = self.evaluate_guard(
                transition.guard.as_ref(),
                instance.subject_id,
                &instance.subject_type,
            ).await?;
            
            all_blockers.extend(result.blockers);
        }
        
        // Deduplicate
        all_blockers.sort_by(|a, b| a.description.cmp(&b.description));
        all_blockers.dedup_by(|a, b| a.description == b.description);
        
        Ok(all_blockers)
    }
    
    async fn get_available_transitions(
        &self,
        instance: &WorkflowInstance,
        def: &WorkflowDefinition,
    ) -> Result<Vec<AvailableTransition>, WorkflowError> {
        let mut transitions = Vec::new();
        
        for t in def.transitions_from(&instance.current_state) {
            let result = self.evaluate_guard(
                t.guard.as_ref(),
                instance.subject_id,
                &instance.subject_type,
            ).await?;
            
            transitions.push(AvailableTransition {
                to_state: t.to.clone(),
                is_auto: t.auto,
                is_manual: t.manual,
                can_transition: result.passed,
                blockers: result.blockers,
            });
        }
        
        Ok(transitions)
    }
    
    fn calculate_progress(&self, instance: &WorkflowInstance, def: &WorkflowDefinition) -> f32 {
        let total = def.states.len() as f32;
        let terminal_count = def.terminal_states().len() as f32;
        
        if def.terminal_states().contains(&instance.current_state.as_str()) {
            return 100.0;
        }
        
        let completed = instance.history.len() as f32;
        let estimated_total = total - terminal_count;
        
        ((completed / estimated_total) * 100.0).min(99.0)
    }
    
    async fn save_instance(&self, instance: &WorkflowInstance) -> Result<(), WorkflowError> {
        sqlx::query(r#"
            INSERT INTO "ob-poc".workflow_instances 
            (instance_id, workflow_id, version, subject_type, subject_id,
             current_state, state_entered_at, history, blockers, 
             created_at, updated_at, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (instance_id) DO UPDATE SET
                current_state = EXCLUDED.current_state,
                state_entered_at = EXCLUDED.state_entered_at,
                history = EXCLUDED.history,
                blockers = EXCLUDED.blockers,
                updated_at = EXCLUDED.updated_at
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
        let row = sqlx::query(r#"
            SELECT instance_id, workflow_id, version, subject_type, subject_id,
                   current_state, state_entered_at, history, blockers,
                   created_at, updated_at, created_by
            FROM "ob-poc".workflow_instances
            WHERE instance_id = $1
        "#)
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(WorkflowError::InstanceNotFound(instance_id))?;
        
        Ok(WorkflowInstance {
            instance_id: row.get("instance_id"),
            workflow_id: row.get("workflow_id"),
            version: row.get::<i32, _>("version") as u32,
            subject_type: row.get("subject_type"),
            subject_id: row.get("subject_id"),
            current_state: row.get("current_state"),
            state_entered_at: row.get("state_entered_at"),
            history: serde_json::from_value(row.get("history")).unwrap_or_default(),
            blockers: serde_json::from_value(row.get("blockers")).unwrap_or_default(),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            created_by: row.get("created_by"),
        })
    }
}

#[derive(Debug)]
struct GuardEvalResult {
    passed: bool,
    blockers: Vec<Blocker>,
}

impl GuardEvalResult {
    fn passed() -> Self {
        Self { passed: true, blockers: vec![] }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkflowStatus {
    pub instance_id: Uuid,
    pub workflow_id: String,
    pub subject_type: String,
    pub subject_id: Uuid,
    pub current_state: String,
    pub state_description: Option<String>,
    pub is_terminal: bool,
    pub blockers: Vec<Blocker>,
    pub available_actions: Vec<AvailableAction>,
    pub available_transitions: Vec<AvailableTransition>,
    pub progress: f32,
    pub history: Vec<super::instance::StateTransition>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableAction {
    pub verb: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AvailableTransition {
    pub to_state: String,
    pub is_auto: bool,
    pub is_manual: bool,
    pub can_transition: bool,
    pub blockers: Vec<Blocker>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Unknown workflow: {0}")]
    UnknownWorkflow(String),
    
    #[error("No initial state defined")]
    NoInitialState,
    
    #[error("Workflow instance not found: {0}")]
    InstanceNotFound(Uuid),
    
    #[error("Invalid transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    
    #[error("Guard failed")]
    GuardFailed { blockers: Vec<Blocker> },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}
```

---

## Part 7: CLI Tooling

### 7.1 `rust/xtask/src/workflow.rs`

```rust
//! CLI commands for workflow management

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum WorkflowCommand {
    /// Create a new workflow from template
    New {
        /// Workflow name (snake_case)
        name: String,
        /// Template: kyc, fund, corporate, or minimal
        #[arg(short, long, default_value = "minimal")]
        template: String,
    },
    /// Validate all workflow definitions
    Validate,
    /// List all workflows and their states
    List,
    /// Show workflow diagram (mermaid)
    Diagram {
        /// Workflow name
        name: String,
    },
    /// List available guards (declarative + custom)
    Guards,
}

pub fn handle_command(cmd: WorkflowCommand) -> Result<()> {
    match cmd {
        WorkflowCommand::New { name, template } => create_workflow(&name, &template),
        WorkflowCommand::Validate => validate_workflows(),
        WorkflowCommand::List => list_workflows(),
        WorkflowCommand::Diagram { name } => show_diagram(&name),
        WorkflowCommand::Guards => list_guards(),
    }
}

fn create_workflow(name: &str, template: &str) -> Result<()> {
    let path = PathBuf::from(format!("rust/config/workflows/{}.yaml", name));
    
    if path.exists() {
        anyhow::bail!("Workflow {} already exists", name);
    }
    
    let content = match template {
        "minimal" => minimal_template(name),
        "kyc" => kyc_template(name),
        "fund" => fund_template(name),
        "corporate" => corporate_template(name),
        _ => anyhow::bail!("Unknown template: {}", template),
    };
    
    std::fs::write(&path, content)?;
    println!("Created workflow: {}", path.display());
    println!("\nNext steps:");
    println!("1. Edit {} to customize states and transitions", path.display());
    println!("2. Run `cargo xtask workflow validate` to check");
    println!("3. Run `cargo xtask workflow diagram {}` to visualize", name);
    
    Ok(())
}

fn minimal_template(name: &str) -> String {
    format!(r#"# Workflow: {name}
# Generated by: cargo xtask workflow new {name}
# Docs: See rust/config/workflows/schema/workflow.schema.json

workflow: {name}
version: 1
description: TODO - describe this workflow

# When does this workflow start?
trigger:
  on: cbu.created
  when:
    cbu_type: CORPORATE  # TODO: adjust trigger conditions

# Define your states
states:
  INTAKE:
    initial: true
    description: Initial state
    
  PROCESSING:
    description: Main processing state
    
  APPROVED:
    terminal: true
    description: Successfully completed
    
  REJECTED:
    terminal: true
    description: Rejected

# Define valid transitions
transitions:
  - from: INTAKE
    to: PROCESSING
    auto: true  # Auto-advance when guard passes
    guard:
      all:
        - role_count:
            role: DIRECTOR
            min: 1
            
  - from: PROCESSING
    to: APPROVED
    manual: true  # Requires explicit action
    
  - from: PROCESSING
    to: REJECTED
    manual: true

# Actions available at each state
actions:
  INTAKE:
    - verb: cbu.assign-role
      description: Add a person with a role
      
  PROCESSING:
    - verb: document.upload
      description: Upload a document
"#)
}

fn validate_workflows() -> Result<()> {
    let dir = PathBuf::from("rust/config/workflows");
    let mut errors = Vec::new();
    let mut count = 0;
    
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().map(|e| e == "yaml").unwrap_or(false) {
            count += 1;
            let content = std::fs::read_to_string(&path)?;
            
            match serde_yaml::from_str::<ob_poc::workflow::WorkflowDefinition>(&content) {
                Ok(def) => {
                    if let Err(e) = def.validate() {
                        errors.push(format!("{}: {}", path.display(), e));
                    }
                }
                Err(e) => {
                    errors.push(format!("{}: Parse error: {}", path.display(), e));
                }
            }
        }
    }
    
    if errors.is_empty() {
        println!("✓ All {} workflows valid", count);
        Ok(())
    } else {
        for e in &errors {
            eprintln!("✗ {}", e);
        }
        anyhow::bail!("{} workflow(s) have errors", errors.len())
    }
}

fn show_diagram(name: &str) -> Result<()> {
    let path = PathBuf::from(format!("rust/config/workflows/{}.yaml", name));
    let content = std::fs::read_to_string(&path)?;
    let def: ob_poc::workflow::WorkflowDefinition = serde_yaml::from_str(&content)?;
    
    println!("```mermaid");
    println!("stateDiagram-v2");
    
    for (state, s) in &def.states {
        if s.initial {
            println!("    [*] --> {}", state);
        }
        if s.terminal {
            println!("    {} --> [*]", state);
        }
    }
    
    for t in &def.transitions {
        let label = if t.auto { " : auto" } else if t.manual { " : manual" } else { "" };
        println!("    {} --> {}{}", t.from, t.to, label);
    }
    
    println!("```");
    
    Ok(())
}

fn list_guards() -> Result<()> {
    println!("Declarative Rules (YAML):");
    println!("  role_count: {{ role: DIRECTOR, min: 1, max: 5 }}");
    println!("  ownership_complete: 100");
    println!("  all_participants_screened: true");
    println!("  no_unresolved_alerts: true");
    println!("  all_ubos_verified: true");
    println!("  documents_present: [PASSPORT, PROOF_OF_ADDRESS]");
    println!("  per_role_documents: {{ role: DIRECTOR, documents: [PASSPORT] }}");
    println!("  case_status: APPROVED");
    println!("  field_equals: {{ field: jurisdiction, value: LU }}");
    println!();
    println!("Custom Guards (Code):");
    
    let registry = ob_poc::workflow::guards::GuardRegistry::new();
    for (name, desc) in registry.list() {
        println!("  {}: {}", name, desc);
    }
    
    Ok(())
}
```

---

## Part 8: MCP Integration

Add to MCP handlers:

```rust
// In mcp/handlers.rs

async fn workflow_status(&self, args: Value) -> Result<Value> {
    let subject_type = args["subject_type"].as_str().ok_or("missing subject_type")?;
    let subject_id: Uuid = args["subject_id"].as_str()
        .ok_or("missing subject_id")?
        .parse()?;
    
    // Find workflow instance for this subject
    let instance_id = self.find_workflow_instance(subject_type, subject_id).await?;
    
    let status = self.workflow_engine.status(instance_id).await?;
    
    Ok(serde_json::to_value(status)?)
}

async fn workflow_advance(&self, args: Value) -> Result<Value> {
    let subject_type = args["subject_type"].as_str().ok_or("missing subject_type")?;
    let subject_id: Uuid = args["subject_id"].as_str()
        .ok_or("missing subject_id")?
        .parse()?;
    
    let instance_id = self.find_workflow_instance(subject_type, subject_id).await?;
    
    let instance = self.workflow_engine.try_advance(instance_id).await?;
    let status = self.workflow_engine.status(instance_id).await?;
    
    Ok(serde_json::to_value(status)?)
}

async fn workflow_transition(&self, args: Value) -> Result<Value> {
    let subject_type = args["subject_type"].as_str().ok_or("missing subject_type")?;
    let subject_id: Uuid = args["subject_id"].as_str()
        .ok_or("missing subject_id")?
        .parse()?;
    let to_state = args["to_state"].as_str().ok_or("missing to_state")?;
    let reason = args["reason"].as_str().map(String::from);
    
    let instance_id = self.find_workflow_instance(subject_type, subject_id).await?;
    
    let instance = self.workflow_engine.transition(
        instance_id,
        to_state,
        None,  // TODO: get user from context
        reason,
    ).await?;
    
    let status = self.workflow_engine.status(instance_id).await?;
    
    Ok(serde_json::to_value(status)?)
}
```

---

## Part 9: Database Schema

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

CREATE INDEX idx_wf_subject ON "ob-poc".workflow_instances (subject_type, subject_id);
CREATE INDEX idx_wf_state ON "ob-poc".workflow_instances (workflow_id, current_state);
CREATE INDEX idx_wf_updated ON "ob-poc".workflow_instances (updated_at DESC);
```

---

## How to Author a New Workflow

### Option 1: CLI Scaffold (Recommended for New Workflows)

```bash
# Create from template
cargo xtask workflow new periodic_review --template kyc

# This creates rust/config/workflows/periodic_review.yaml
# with a reasonable starting structure
```

### Option 2: Copy and Modify (Recommended for Similar Workflows)

```bash
cp rust/config/workflows/kyc_onboarding.yaml rust/config/workflows/enhanced_due_diligence.yaml
# Edit the new file
```

### Option 3: Write from Scratch

1. Create `rust/config/workflows/my_workflow.yaml`
2. IDE provides auto-completion from JSON schema
3. Run `cargo xtask workflow validate` to check
4. Run `cargo xtask workflow diagram my_workflow` to visualize

### Validation Before Deploy

```bash
# Validate all workflows
cargo xtask workflow validate

# See available guards
cargo xtask workflow guards

# Visualize
cargo xtask workflow diagram kyc_onboarding
```

---

## Implementation Checklist

- [ ] Create database migration for workflow_instances
- [ ] Create JSON schema for workflow YAML
- [ ] Implement `definition.rs` - parse YAML
- [ ] Implement `instance.rs` - workflow state
- [ ] Implement `blocker.rs` - blocker types
- [ ] Implement `rules/mod.rs` - rule evaluator
- [ ] Implement `rules/role_count.rs`
- [ ] Implement `rules/ownership.rs`
- [ ] Implement `rules/screening.rs`
- [ ] Implement `rules/documents.rs`
- [ ] Implement `rules/case_status.rs`
- [ ] Implement `guards/mod.rs` - guard registry
- [ ] Implement `guards/fund.rs`
- [ ] Implement `guards/corporate.rs`
- [ ] Implement `guards/ubo.rs`
- [ ] Implement `engine.rs` - core engine
- [ ] Implement `loader.rs` - load YAML on startup
- [ ] Create `kyc_onboarding.yaml` workflow
- [ ] Create `fund_onboarding.yaml` workflow
- [ ] Add xtask CLI commands
- [ ] Add MCP tools (workflow_status, workflow_advance, workflow_transition)
- [ ] Wire MCP handlers
- [ ] Test full KYC workflow end-to-end
- [ ] Test auto-advance behavior
- [ ] Test blocker resolution

---

## Effort Estimate

| Component | Hours |
|-----------|-------|
| Types (definition, instance, blocker) | 2 |
| Rule evaluator + rules | 4 |
| Custom guard registry + guards | 3 |
| Workflow engine | 4 |
| YAML loader + validation | 2 |
| Database schema + repository | 1 |
| CLI tooling | 2 |
| MCP integration | 2 |
| Testing | 2 |
| **Total** | **~22 hours** |
