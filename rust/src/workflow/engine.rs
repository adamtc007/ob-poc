//! Workflow Engine
//!
//! Core workflow execution logic. Manages state transitions, guard evaluation,
//! and blocker tracking.

use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::definition::WorkflowDefinition;
use super::guards::GuardEvaluator;
use super::repository::WorkflowRepository;
use super::state::{Blocker, StateTransition, WorkflowInstance};
use super::WorkflowError;

/// The workflow execution engine
pub struct WorkflowEngine {
    repo: WorkflowRepository,
    guard_evaluator: GuardEvaluator,
    definitions: Arc<HashMap<String, WorkflowDefinition>>,
}

impl WorkflowEngine {
    /// Create a new workflow engine
    pub fn new(pool: PgPool, definitions: HashMap<String, WorkflowDefinition>) -> Self {
        Self {
            repo: WorkflowRepository::new(pool.clone()),
            guard_evaluator: GuardEvaluator::new(pool),
            definitions: Arc::new(definitions),
        }
    }

    /// Get the workflow definitions
    pub fn definitions(&self) -> &HashMap<String, WorkflowDefinition> {
        &self.definitions
    }

    /// Start a new workflow instance
    pub async fn start_workflow(
        &self,
        workflow_id: &str,
        subject_type: &str,
        subject_id: Uuid,
        created_by: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let definition = self
            .definitions
            .get(workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(workflow_id.to_string()))?;

        let initial_state = definition
            .initial_state()
            .ok_or(WorkflowError::NoInitialState)?;

        let instance = WorkflowInstance::new(
            workflow_id.to_string(),
            definition.version,
            subject_type.to_string(),
            subject_id,
            initial_state.to_string(),
            created_by.clone(),
        );

        // Persist
        self.repo.save(&instance).await?;

        // Log the initial state entry
        self.repo
            .log_transition(
                instance.instance_id,
                None,
                &instance.current_state,
                "system",
                created_by.as_deref(),
                Some("Workflow started"),
                &[],
            )
            .await?;

        // Immediately try to advance (for auto transitions from initial state)
        self.try_advance(instance.instance_id).await
    }

    /// Get current workflow status with blockers and available actions
    pub async fn get_status(&self, instance_id: Uuid) -> Result<WorkflowStatus, WorkflowError> {
        let instance = self.repo.load(instance_id).await?;
        let definition = self
            .definitions
            .get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;

        // Evaluate current blockers
        let blockers = self.evaluate_blockers(&instance, definition).await?;

        // Get available transitions
        let available_transitions = self
            .get_available_transitions(&instance, definition)
            .await?;

        // Get available actions
        let available_actions: Vec<AvailableAction> = definition
            .actions_for_state(&instance.current_state)
            .into_iter()
            .map(|a| AvailableAction {
                action: a.action.clone(),
                verb: a.verb.clone(),
                description: a.description.clone(),
            })
            .collect();

        let state_def = definition.states.get(&instance.current_state);

        Ok(WorkflowStatus {
            instance_id: instance.instance_id,
            workflow_id: instance.workflow_id.clone(),
            subject_type: instance.subject_type.clone(),
            subject_id: instance.subject_id,
            current_state: instance.current_state.clone(),
            state_description: state_def.map(|s| s.description.clone()),
            is_terminal: state_def.map(|s| s.terminal).unwrap_or(false),
            blockers,
            available_transitions,
            available_actions,
            progress: self.calculate_progress(&instance, definition),
            history: instance.history.clone(),
        })
    }

    /// Try to automatically advance the workflow
    pub async fn try_advance(&self, instance_id: Uuid) -> Result<WorkflowInstance, WorkflowError> {
        let mut instance = self.repo.load(instance_id).await?;
        let definition = self
            .definitions
            .get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;

        // Find auto transitions from current state
        let auto_transitions: Vec<_> = definition
            .transitions_from(&instance.current_state)
            .into_iter()
            .filter(|t| t.auto)
            .collect();

        for transition in auto_transitions {
            let can_transition = if let Some(guard) = &transition.guard {
                let result = self
                    .guard_evaluator
                    .evaluate(guard, instance.subject_id, &instance.subject_type)
                    .await
                    .map_err(WorkflowError::Database)?;

                if result.passed {
                    true
                } else {
                    // Update blockers
                    instance.blockers = result.blockers;
                    self.repo.save(&instance).await?;
                    false
                }
            } else {
                // No guard means always pass
                true
            };

            if can_transition {
                // Execute transition
                instance = self
                    .execute_transition(instance, &transition.to, None, None)
                    .await?;

                // Recursively try to advance again (boxed to avoid infinite future size)
                return Box::pin(self.try_advance(instance.instance_id)).await;
            }
        }

        Ok(instance)
    }

    /// Manually transition to a specific state
    pub async fn transition(
        &self,
        instance_id: Uuid,
        to_state: &str,
        by: Option<String>,
        reason: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let instance = self.repo.load(instance_id).await?;
        let definition = self
            .definitions
            .get(&instance.workflow_id)
            .ok_or_else(|| WorkflowError::UnknownWorkflow(instance.workflow_id.clone()))?;

        // Validate transition exists
        let transition = definition
            .get_transition(&instance.current_state, to_state)
            .ok_or_else(|| WorkflowError::InvalidTransition {
                from: instance.current_state.clone(),
                to: to_state.to_string(),
            })?;

        // Check guard if present
        if let Some(guard) = &transition.guard {
            let result = self
                .guard_evaluator
                .evaluate(guard, instance.subject_id, &instance.subject_type)
                .await
                .map_err(WorkflowError::Database)?;

            if !result.passed {
                return Err(WorkflowError::GuardFailed {
                    guard: guard.clone(),
                    blockers: result.blockers,
                });
            }
        }

        self.execute_transition(instance, to_state, by, reason)
            .await
    }

    /// Execute a state transition
    async fn execute_transition(
        &self,
        mut instance: WorkflowInstance,
        to_state: &str,
        by: Option<String>,
        reason: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        let from_state = instance.current_state.clone();

        instance.transition_to(to_state.to_string(), by.clone(), reason.clone());

        self.repo.save(&instance).await?;

        // Log to audit trail
        self.repo
            .log_transition(
                instance.instance_id,
                Some(&from_state),
                to_state,
                if by.is_some() { "manual" } else { "auto" },
                by.as_deref(),
                reason.as_deref(),
                &instance.blockers,
            )
            .await?;

        Ok(instance)
    }

    /// Evaluate all blockers for current state
    async fn evaluate_blockers(
        &self,
        instance: &WorkflowInstance,
        definition: &WorkflowDefinition,
    ) -> Result<Vec<Blocker>, WorkflowError> {
        // Find transitions from current state
        let outgoing = definition.transitions_from(&instance.current_state);

        let mut all_blockers = Vec::new();

        for transition in outgoing {
            if let Some(guard) = &transition.guard {
                let result = self
                    .guard_evaluator
                    .evaluate(guard, instance.subject_id, &instance.subject_type)
                    .await
                    .map_err(WorkflowError::Database)?;

                if !result.passed {
                    all_blockers.extend(result.blockers);
                }
            }
        }

        // Deduplicate blockers by description
        all_blockers.sort_by(|a, b| a.description.cmp(&b.description));
        all_blockers.dedup_by(|a, b| a.description == b.description);

        Ok(all_blockers)
    }

    /// Get available transitions from current state
    async fn get_available_transitions(
        &self,
        instance: &WorkflowInstance,
        definition: &WorkflowDefinition,
    ) -> Result<Vec<AvailableTransition>, WorkflowError> {
        let outgoing = definition.transitions_from(&instance.current_state);
        let mut transitions = Vec::new();

        for t in outgoing {
            let guard_status = if let Some(guard) = &t.guard {
                let result = self
                    .guard_evaluator
                    .evaluate(guard, instance.subject_id, &instance.subject_type)
                    .await
                    .map_err(WorkflowError::Database)?;

                if result.passed {
                    GuardStatus::Passed
                } else {
                    GuardStatus::Blocked {
                        blockers: result.blockers,
                    }
                }
            } else {
                GuardStatus::NoGuard
            };

            let state_def = definition.states.get(&t.to);

            transitions.push(AvailableTransition {
                to_state: t.to.clone(),
                description: t
                    .description
                    .clone()
                    .or_else(|| state_def.map(|s| s.description.clone()))
                    .unwrap_or_default(),
                is_manual: t.manual,
                guard_status,
            });
        }

        Ok(transitions)
    }

    /// Calculate progress percentage
    fn calculate_progress(
        &self,
        instance: &WorkflowInstance,
        definition: &WorkflowDefinition,
    ) -> f32 {
        let terminal_states = definition.terminal_states();

        // If in terminal state, 100%
        if terminal_states.contains(&instance.current_state) {
            return 100.0;
        }

        // Simple heuristic: count completed transitions vs estimated total
        let total_states = definition.states.len() as f32;
        let completed = instance.history.len() as f32;
        let estimated_total = total_states - terminal_states.len() as f32;

        if estimated_total > 0.0 {
            ((completed / estimated_total) * 100.0).min(99.0) // Cap at 99% until terminal
        } else {
            0.0
        }
    }

    /// Find or create workflow for a subject
    pub async fn find_or_start(
        &self,
        workflow_id: &str,
        subject_type: &str,
        subject_id: Uuid,
        created_by: Option<String>,
    ) -> Result<WorkflowInstance, WorkflowError> {
        if let Some(instance) = self
            .repo
            .find_by_subject(workflow_id, subject_type, subject_id)
            .await?
        {
            Ok(instance)
        } else {
            self.start_workflow(workflow_id, subject_type, subject_id, created_by)
                .await
        }
    }
}

/// Full workflow status with blockers and available actions
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStatus {
    pub instance_id: Uuid,
    pub workflow_id: String,
    pub subject_type: String,
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

/// Available transition from current state
#[derive(Debug, Clone, Serialize)]
pub struct AvailableTransition {
    pub to_state: String,
    pub description: String,
    pub is_manual: bool,
    pub guard_status: GuardStatus,
}

/// Status of a guard evaluation
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum GuardStatus {
    Passed,
    Blocked { blockers: Vec<Blocker> },
    NoGuard,
}

/// Available action at current state
#[derive(Debug, Clone, Serialize)]
pub struct AvailableAction {
    pub action: String,
    pub verb: String,
    pub description: String,
}
