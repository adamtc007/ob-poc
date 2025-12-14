//! Workflow Definition Types and YAML Loading
//!
//! Workflows are defined in YAML files and loaded at startup.
//! Definitions are also cached to the database for fast querying.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::Path;

use super::WorkflowError;

/// A complete workflow definition loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Workflow identifier
    pub workflow: String,
    /// Version number
    #[serde(default = "default_version")]
    pub version: u32,
    /// Human-readable description
    #[serde(default)]
    pub description: String,

    /// What triggers this workflow
    #[serde(default)]
    pub trigger: Option<TriggerDef>,

    /// State definitions
    pub states: HashMap<String, StateDef>,

    /// Valid transitions between states
    #[serde(default)]
    pub transitions: Vec<TransitionDef>,

    /// Requirements for each state
    #[serde(default)]
    pub requirements: HashMap<String, Vec<RequirementDef>>,

    /// Actions available at each state
    #[serde(default)]
    pub actions: HashMap<String, Vec<ActionDef>>,
}

fn default_version() -> u32 {
    1
}

fn default_min_one() -> u32 {
    1
}

fn default_ubo_threshold() -> f64 {
    25.0
}

fn default_max_age_days() -> u32 {
    90
}

impl WorkflowDefinition {
    /// Get the initial state for this workflow
    pub fn initial_state(&self) -> Option<&str> {
        self.states
            .iter()
            .find(|(_, s)| s.initial)
            .map(|(name, _)| name.as_str())
    }

    /// Get terminal states
    pub fn terminal_states(&self) -> Vec<String> {
        self.states
            .iter()
            .filter(|(_, s)| s.terminal)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get transitions from a specific state
    pub fn transitions_from(&self, state: &str) -> Vec<&TransitionDef> {
        self.transitions
            .iter()
            .filter(|t| t.from == state)
            .collect()
    }

    /// Get available actions for a state
    pub fn actions_for_state(&self, state: &str) -> Vec<&ActionDef> {
        self.actions
            .get(state)
            .map(|a| a.iter().collect())
            .unwrap_or_default()
    }

    /// Check if a transition is valid
    pub fn is_valid_transition(&self, from: &str, to: &str) -> bool {
        self.transitions
            .iter()
            .any(|t| t.from == from && t.to == to)
    }

    /// Get transition definition
    pub fn get_transition(&self, from: &str, to: &str) -> Option<&TransitionDef> {
        self.transitions
            .iter()
            .find(|t| t.from == from && t.to == to)
    }
}

/// Workflow trigger definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDef {
    /// Event that triggers this workflow (e.g., "cbu.created")
    pub event: String,
    /// Conditions that must be met
    #[serde(default)]
    pub conditions: Vec<TriggerCondition>,
}

/// A trigger condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    /// Field to check
    pub field: String,
    /// Values that match (OR)
    #[serde(rename = "in", default)]
    pub in_values: Vec<String>,
    /// Value that must match
    #[serde(default)]
    pub equals: Option<String>,
}

/// State definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDef {
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Is this the initial state?
    #[serde(default)]
    pub initial: bool,
    /// Is this a terminal state?
    #[serde(default)]
    pub terminal: bool,
    /// Timeout for this state (optional)
    #[serde(default)]
    pub timeout_hours: Option<u32>,
}

/// Transition definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionDef {
    /// Source state
    pub from: String,
    /// Target state
    pub to: String,
    /// Guard function name (must pass to allow transition)
    #[serde(default)]
    pub guard: Option<String>,
    /// Is this an automatic transition (happens when guard passes)?
    #[serde(default)]
    pub auto: bool,
    /// Requires manual action?
    #[serde(default)]
    pub manual: bool,
    /// Description of this transition
    #[serde(default)]
    pub description: Option<String>,
}

/// Condition for conditional requirements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalCheck {
    /// Field to check
    pub field: String,
    /// Exact value match
    #[serde(default)]
    pub equals: Option<String>,
    /// Match any of these values
    #[serde(rename = "in", default)]
    pub in_values: Vec<String>,
}

/// Requirement definition for a state
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequirementDef {
    // --- Core entity requirements ---
    /// Minimum count of a role
    RoleCount {
        role: String,
        #[serde(default = "default_min_one")]
        min: u32,
        #[serde(default)]
        description: String,
    },

    /// Check that specified fields have non-null values
    FieldPresent {
        fields: Vec<String>,
        #[serde(default)]
        description: String,
    },

    /// At least N products assigned
    ProductAssigned {
        #[serde(default = "default_min_one")]
        min: u32,
        #[serde(default)]
        description: String,
    },

    /// Check relationship exists (e.g., MANAGEMENT_COMPANY, UMBRELLA)
    RelationshipExists {
        relationship_type: String,
        #[serde(default)]
        description: String,
    },

    /// Apply requirement only when condition is met
    Conditional {
        condition: ConditionalCheck,
        requirement: Box<RequirementDef>,
        #[serde(default)]
        description: String,
    },

    // --- Document requirements ---
    /// Required document set at CBU level
    DocumentSet {
        documents: Vec<String>,
        #[serde(default)]
        description: String,
    },

    /// Document required per entity of a type
    PerEntityDocument {
        entity_type: String,
        documents: Vec<String>,
        #[serde(default)]
        description: String,
    },

    /// All documents reviewed
    DocumentsReviewed {
        #[serde(default)]
        description: String,
    },

    // --- Screening requirements ---
    /// All entities must be screened
    AllEntitiesScreened {
        #[serde(default)]
        description: String,
    },

    /// All screenings run within N days
    AllScreeningsCurrent {
        #[serde(default = "default_max_age_days")]
        max_age_days: u32,
        #[serde(default)]
        description: String,
    },

    /// All screenings complete (not pending)
    AllScreeningsComplete {
        #[serde(default)]
        description: String,
    },

    /// No open alerts/red flags
    NoOpenAlerts {
        #[serde(default)]
        description: String,
    },

    /// No open red flags
    NoOpenRedFlags {
        #[serde(default)]
        description: String,
    },

    /// No pending screening hits
    NoPendingHits {
        #[serde(default)]
        description: String,
    },

    // --- UBO requirements ---
    /// Ownership must sum to threshold
    OwnershipComplete {
        #[serde(default = "default_ubo_threshold")]
        threshold: f64,
        #[serde(default)]
        description: String,
    },

    /// All UBOs must be verified
    AllUbosVerified {
        #[serde(default)]
        description: String,
    },

    /// All UBOs screened
    AllUbosScreened {
        #[serde(default)]
        description: String,
    },

    /// All UBOs have identity documents
    AllUbosHaveIdentityDocs {
        #[serde(default)]
        description: String,
    },

    /// All ownership chains traced to natural persons
    ChainsResolvedToPersons {
        #[serde(default)]
        description: String,
    },

    /// UBO threshold applied and UBOs identified
    UboThresholdApplied {
        #[serde(default = "default_ubo_threshold")]
        threshold: f64,
        #[serde(default)]
        description: String,
    },

    /// UBO register complete
    UboRegisterComplete {
        #[serde(default)]
        description: String,
    },

    /// No unknown owners in chains
    NoUnknownOwners {
        #[serde(default)]
        description: String,
    },

    /// Exemptions documented for non-person chain terminations
    ExemptionsDocumented {
        #[serde(default)]
        description: String,
    },

    /// Determination rationale recorded
    DeterminationRationaleRecorded {
        #[serde(default)]
        description: String,
    },

    // --- Case requirements ---
    /// KYC case exists for this subject
    CaseExists {
        #[serde(default)]
        case_type: Option<String>,
        #[serde(default)]
        description: String,
    },

    /// Analyst assigned to case
    AnalystAssigned {
        #[serde(default)]
        description: String,
    },

    /// Risk rating has been set
    RiskRatingSet {
        #[serde(default)]
        description: String,
    },

    /// Case checklist complete
    CaseChecklistComplete {
        #[serde(default)]
        description: String,
    },

    /// Generic checklist complete
    ChecklistComplete {
        #[serde(default)]
        description: String,
    },

    /// Case approval recorded
    ApprovalRecorded {
        #[serde(default)]
        description: String,
    },

    /// Case rejection recorded
    RejectionRecorded {
        #[serde(default)]
        description: String,
    },

    // --- Workstream requirements ---
    /// Entity workstreams created for all linked entities
    EntityWorkstreamsCreated {
        #[serde(default)]
        description: String,
    },

    /// All workstreams have required data
    AllWorkstreamsDataComplete {
        #[serde(default)]
        description: String,
    },

    /// All entities have types assigned
    AllEntitiesTyped {
        #[serde(default)]
        description: String,
    },

    // --- Data freshness ---
    /// Entity data refreshed within N days
    EntityDataCurrent {
        #[serde(default = "default_max_age_days")]
        max_age_days: u32,
        #[serde(default)]
        description: String,
    },

    /// Change log reviewed
    ChangeLogReviewed {
        #[serde(default)]
        description: String,
    },

    /// Risk reassessment complete
    RiskReassessmentComplete {
        #[serde(default)]
        description: String,
    },

    // --- Sign-off and scheduling ---
    /// Sign-off recorded
    SignOffRecorded {
        #[serde(default)]
        description: String,
    },

    /// Next review date scheduled
    NextReviewScheduled {
        #[serde(default)]
        description: String,
    },

    // --- Deferral ---
    /// Deferral reason documented
    DeferralReasonDocumented {
        #[serde(default)]
        description: String,
    },

    /// Deferral approval recorded
    DeferralApprovalRecorded {
        #[serde(default)]
        description: String,
    },

    /// Custom requirement
    Custom {
        code: String,
        #[serde(default)]
        params: HashMap<String, serde_json::Value>,
        #[serde(default)]
        description: String,
    },
}

/// Action available at a state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    /// Action identifier
    pub action: String,
    /// DSL verb to execute
    pub verb: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Required parameters
    #[serde(default)]
    pub params: Vec<String>,
}

/// Loader for workflow definitions
pub struct WorkflowLoader;

impl WorkflowLoader {
    /// Load all workflow definitions from a directory
    pub fn load_from_dir(dir: &Path) -> Result<HashMap<String, WorkflowDefinition>, WorkflowError> {
        let mut definitions = HashMap::new();

        if !dir.exists() {
            return Ok(definitions);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path
                .extension()
                .map(|e| e == "yaml" || e == "yml")
                .unwrap_or(false)
            {
                let content = std::fs::read_to_string(&path)?;
                let def: WorkflowDefinition = serde_yaml::from_str(&content)?;
                definitions.insert(def.workflow.clone(), def);
            }
        }

        Ok(definitions)
    }

    /// Load workflows from YAML and sync to database
    pub async fn load_and_sync(
        dir: &Path,
        pool: &PgPool,
    ) -> Result<HashMap<String, WorkflowDefinition>, WorkflowError> {
        let definitions = Self::load_from_dir(dir)?;

        for (workflow_id, def) in &definitions {
            Self::sync_to_db(pool, workflow_id, def).await?;
        }

        Ok(definitions)
    }

    /// Sync a single workflow definition to database
    async fn sync_to_db(
        pool: &PgPool,
        workflow_id: &str,
        def: &WorkflowDefinition,
    ) -> Result<(), WorkflowError> {
        let json = serde_json::to_value(def)?;
        let hash = Self::content_hash(&json);

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".workflow_definitions
            (workflow_id, version, description, definition_json, content_hash, loaded_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (workflow_id) DO UPDATE SET
                version = EXCLUDED.version,
                description = EXCLUDED.description,
                definition_json = EXCLUDED.definition_json,
                content_hash = EXCLUDED.content_hash,
                loaded_at = NOW()
            WHERE workflow_definitions.content_hash != EXCLUDED.content_hash
        "#,
        )
        .bind(workflow_id)
        .bind(def.version as i32)
        .bind(&def.description)
        .bind(&json)
        .bind(&hash)
        .execute(pool)
        .await
        .map_err(WorkflowError::Database)?;

        Ok(())
    }

    /// Compute SHA-256 hash of JSON content
    fn content_hash(json: &serde_json::Value) -> String {
        let content = serde_json::to_string(json).unwrap_or_default();
        let hash = Sha256::digest(content.as_bytes());
        format!("{:x}", hash)
    }

    /// Load a single workflow definition from a file
    pub fn load_from_file(path: &Path) -> Result<WorkflowDefinition, WorkflowError> {
        let content = std::fs::read_to_string(path)?;
        let def: WorkflowDefinition = serde_yaml::from_str(&content)?;
        Ok(def)
    }

    /// Load from a YAML string
    pub fn load_from_str(yaml: &str) -> Result<WorkflowDefinition, WorkflowError> {
        let def: WorkflowDefinition = serde_yaml::from_str(yaml)?;
        Ok(def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WORKFLOW: &str = r#"
workflow: test_workflow
version: 1
description: Test workflow

states:
  INTAKE:
    description: Initial state
    initial: true
  PROCESSING:
    description: Processing state
  COMPLETE:
    description: Done
    terminal: true

transitions:
  - from: INTAKE
    to: PROCESSING
    auto: true
  - from: PROCESSING
    to: COMPLETE
    guard: processing_complete
    manual: true

requirements:
  PROCESSING:
    - type: role_count
      role: DIRECTOR
      min: 1
      description: Need a director

actions:
  INTAKE:
    - action: add_data
      verb: entity.create
      description: Add initial data
"#;

    #[test]
    fn test_parse_workflow() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();

        assert_eq!(def.workflow, "test_workflow");
        assert_eq!(def.version, 1);
        assert_eq!(def.states.len(), 3);
        assert_eq!(def.transitions.len(), 2);
    }

    #[test]
    fn test_initial_state() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();
        assert_eq!(def.initial_state(), Some("INTAKE"));
    }

    #[test]
    fn test_terminal_states() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();
        let terminals = def.terminal_states();
        assert_eq!(terminals, vec!["COMPLETE"]);
    }

    #[test]
    fn test_transitions_from() {
        let def = WorkflowLoader::load_from_str(SAMPLE_WORKFLOW).unwrap();
        let transitions = def.transitions_from("INTAKE");
        assert_eq!(transitions.len(), 1);
        assert_eq!(transitions[0].to, "PROCESSING");
    }

    #[test]
    fn test_conditional_requirement() {
        let yaml = r#"
workflow: test
states:
  S1:
    initial: true
transitions: []
requirements:
  S1:
    - type: conditional
      condition:
        field: client_type
        in: [FUND, SUB_FUND]
      requirement:
        type: relationship_exists
        relationship_type: MANAGEMENT_COMPANY
      description: Funds need ManCo
"#;
        let def = WorkflowLoader::load_from_str(yaml).unwrap();
        assert_eq!(def.requirements.get("S1").unwrap().len(), 1);
    }
}
