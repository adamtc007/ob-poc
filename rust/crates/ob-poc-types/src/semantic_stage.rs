//! Semantic Stage Types
//!
//! Types for the semantic stage map - a session-time view that helps
//! the agent reason about "where are we in the onboarding journey."
//!
//! Key distinction:
//! - SemanticStageMap: Configuration (loaded from YAML, static)
//! - SemanticState: Derived at runtime (computed from entity tables, not stored)
//!
//! This is NOT a workflow engine. It's an agent decision-support view.
//! The entities ARE the truth - this view helps pick the right DSL.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// CONFIGURATION TYPES (loaded from YAML)
// ============================================================================

/// The complete semantic stage map (loaded from YAML config)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SemanticStageMap {
    /// Stage definitions in dependency order
    pub stages: Vec<StageDefinition>,
    /// Product -> Stage requirements mapping
    pub product_stages: HashMap<String, ProductStageConfig>,
    /// Entity type -> Stage reverse lookup
    pub entity_stage_mapping: HashMap<String, String>,
    /// Condition definitions for conditional stages
    #[serde(default)]
    pub condition_definitions: HashMap<String, ConditionDefinition>,
}

/// A single stage definition from config
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StageDefinition {
    /// Unique stage code (e.g., "KYC_REVIEW")
    pub code: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this stage accomplishes
    pub description: String,
    /// Entity types required for this stage to be complete
    pub required_entities: Vec<String>,
    /// Stage codes this stage depends on
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// If true, this stage blocks downstream stages until complete
    #[serde(default)]
    pub blocking: bool,
    /// Condition name for conditional stages (e.g., "has_otc_instruments")
    #[serde(default)]
    pub conditional: Option<String>,
    /// DSL verbs relevant to this stage (for agent filtering)
    /// When the user focuses on this stage, the agent prioritizes these verbs
    #[serde(default)]
    pub relevant_verbs: Option<Vec<String>>,
}

/// Product-specific stage requirements
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProductStageConfig {
    /// Stages that are always required for this product
    pub mandatory: Vec<String>,
    /// Stages that are conditionally required
    #[serde(default)]
    pub conditional: Vec<ConditionalStage>,
    /// Additional entities required per stage for this product
    #[serde(default)]
    pub adds_to_stage: HashMap<String, StageAdditions>,
}

/// A conditionally required stage
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionalStage {
    /// Stage code
    pub stage: String,
    /// Condition name that must be true
    pub when: String,
}

/// Additional entities a product adds to a stage
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StageAdditions {
    /// Extra entity types required
    pub extra_entities: Vec<String>,
}

/// Definition of a condition for conditional stages
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConditionDefinition {
    /// Human-readable description
    pub description: String,
    /// Entity type to check
    #[serde(default)]
    pub check_entity: Option<String>,
    /// Field to check on the entity
    #[serde(default)]
    pub check_field: Option<String>,
    /// Values that make the condition true
    #[serde(default)]
    pub check_values: Vec<String>,
    /// Product code to check for
    #[serde(default)]
    pub check_product: Option<String>,
}

// ============================================================================
// RUNTIME TYPES (derived, not stored)
// ============================================================================

/// Runtime semantic state for a specific CBU
/// Derived on-demand, NOT persisted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticState {
    /// CBU this state is for
    pub cbu_id: uuid::Uuid,
    /// CBU name for display
    pub cbu_name: String,
    /// Products subscribed to by this CBU
    pub products: Vec<String>,
    /// All required stages with their current status
    pub required_stages: Vec<StageWithStatus>,
    /// Overall progress summary
    pub overall_progress: Progress,
    /// Stage codes that can be worked on next (dependencies met, not complete)
    pub next_actionable: Vec<String>,
    /// Stage codes that are blocking (must complete before downstream)
    pub blocking_stages: Vec<String>,
    /// Entities missing to complete stages
    pub missing_entities: Vec<MissingEntity>,
}

/// A stage with its current status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageWithStatus {
    /// Stage code
    pub code: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Current status
    pub status: StageStatus,
    /// Entity requirements with their status
    pub required_entities: Vec<EntityStatus>,
    /// Whether this stage blocks downstream stages
    pub is_blocking: bool,
}

/// Status of a stage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// All required entities exist
    Complete,
    /// Some but not all required entities exist
    InProgress,
    /// No required entities exist yet
    NotStarted,
    /// Dependencies not met - can't start yet
    Blocked,
    /// Conditional stage where condition is not met
    NotRequired,
}

/// Status of an entity type requirement within a stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityStatus {
    /// Entity type code
    pub entity_type: String,
    /// Whether this entity is required (vs optional)
    pub required: bool,
    /// Whether at least one instance exists
    pub exists: bool,
    /// Count of existing instances
    pub count: usize,
    /// IDs of existing instances
    pub ids: Vec<uuid::Uuid>,
}

/// A missing entity that would advance a stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingEntity {
    /// Entity type code
    pub entity_type: String,
    /// Stage code this entity belongs to
    pub stage: String,
    /// Stage name for display
    pub stage_name: String,
    /// What creating this entity accomplishes
    pub semantic_purpose: String,
}

/// Progress summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    /// Number of stages complete
    pub stages_complete: usize,
    /// Total number of required stages
    pub stages_total: usize,
    /// Percentage complete (0.0 - 100.0)
    pub percentage: f32,
}

// ============================================================================
// DISPLAY HELPERS
// ============================================================================

impl StageStatus {
    /// Icon for display
    pub fn icon(&self) -> &'static str {
        match self {
            StageStatus::Complete => "✓",
            StageStatus::InProgress => "◐",
            StageStatus::NotStarted => "○",
            StageStatus::Blocked => "⊘",
            StageStatus::NotRequired => "—",
        }
    }

    /// Whether this status represents "done"
    pub fn is_done(&self) -> bool {
        matches!(self, StageStatus::Complete | StageStatus::NotRequired)
    }
}

impl SemanticState {
    /// Format as text for agent prompt injection
    pub fn to_prompt_context(&self) -> String {
        let mut ctx = format!(
            "## Onboarding Context: {}\n\n\
             Products: {}\n\
             Progress: {}/{} stages ({:.0}%)\n\n",
            self.cbu_name,
            if self.products.is_empty() {
                "None selected".to_string()
            } else {
                self.products.join(", ")
            },
            self.overall_progress.stages_complete,
            self.overall_progress.stages_total,
            self.overall_progress.percentage,
        );

        // Stage status
        ctx.push_str("### Stages\n");
        for stage in &self.required_stages {
            ctx.push_str(&format!(
                "{} {} - {}\n",
                stage.status.icon(),
                stage.name,
                stage.description
            ));
        }

        // Blocking stages warning
        if !self.blocking_stages.is_empty() {
            ctx.push_str(&format!(
                "\n⚠️ Blocking: {} must complete before trading can proceed.\n",
                self.blocking_stages.join(", ")
            ));
        }

        // Next actionable
        if !self.next_actionable.is_empty() {
            ctx.push_str(&format!(
                "\nNext actionable: {}\n",
                self.next_actionable.join(", ")
            ));
        }

        // Missing entities (first few)
        if !self.missing_entities.is_empty() {
            ctx.push_str("\n### Missing to proceed\n");
            for gap in self.missing_entities.iter().take(5) {
                ctx.push_str(&format!("- {} (for {})\n", gap.entity_type, gap.stage_name));
            }
            if self.missing_entities.len() > 5 {
                ctx.push_str(&format!(
                    "  ...and {} more\n",
                    self.missing_entities.len() - 5
                ));
            }
        }

        ctx
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_status_icons() {
        assert_eq!(StageStatus::Complete.icon(), "✓");
        assert_eq!(StageStatus::InProgress.icon(), "◐");
        assert_eq!(StageStatus::NotStarted.icon(), "○");
        assert_eq!(StageStatus::Blocked.icon(), "⊘");
        assert_eq!(StageStatus::NotRequired.icon(), "—");
    }

    #[test]
    fn stage_status_is_done() {
        assert!(StageStatus::Complete.is_done());
        assert!(StageStatus::NotRequired.is_done());
        assert!(!StageStatus::InProgress.is_done());
        assert!(!StageStatus::NotStarted.is_done());
        assert!(!StageStatus::Blocked.is_done());
    }

    #[test]
    fn semantic_state_to_prompt() {
        let state = SemanticState {
            cbu_id: uuid::Uuid::nil(),
            cbu_name: "Test Fund".to_string(),
            products: vec!["CUSTODY".to_string()],
            required_stages: vec![
                StageWithStatus {
                    code: "CLIENT_SETUP".to_string(),
                    name: "Client Setup".to_string(),
                    description: "Establish the client".to_string(),
                    status: StageStatus::Complete,
                    required_entities: vec![],
                    is_blocking: false,
                },
                StageWithStatus {
                    code: "KYC_REVIEW".to_string(),
                    name: "KYC Review".to_string(),
                    description: "Know your customer".to_string(),
                    status: StageStatus::NotStarted,
                    required_entities: vec![],
                    is_blocking: true,
                },
            ],
            overall_progress: Progress {
                stages_complete: 1,
                stages_total: 2,
                percentage: 50.0,
            },
            next_actionable: vec!["KYC_REVIEW".to_string()],
            blocking_stages: vec!["KYC_REVIEW".to_string()],
            missing_entities: vec![MissingEntity {
                entity_type: "kyc_case".to_string(),
                stage: "KYC_REVIEW".to_string(),
                stage_name: "KYC Review".to_string(),
                semantic_purpose: "Know your customer".to_string(),
            }],
        };

        let prompt = state.to_prompt_context();
        assert!(prompt.contains("Test Fund"));
        assert!(prompt.contains("CUSTODY"));
        assert!(prompt.contains("1/2 stages"));
        assert!(prompt.contains("✓ Client Setup"));
        assert!(prompt.contains("○ KYC Review"));
        assert!(prompt.contains("Blocking"));
        assert!(prompt.contains("kyc_case"));
    }
}
