//! DSL State Management
//!
//! This module provides state management capabilities for DSL instances,
//! including state tracking, version control, and change event handling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// DSL state manager for tracking DSL instance states
#[derive(Debug)]
pub struct DslStateManager {
    /// Active DSL states
    states: HashMap<Uuid, DslState>,
    /// State change history
    change_history: Vec<StateChangeEvent>,
}

/// DSL instance state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslState {
    /// Instance ID
    pub instance_id: Uuid,
    /// Current version
    pub version: u64,
    /// Current status
    pub status: StateStatus,
    /// Domain context
    pub domain: String,
    /// Current DSL content
    pub current_dsl: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// State metadata
    pub metadata: HashMap<String, String>,
}

/// State status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateStatus {
    /// DSL is being created
    Creating,
    /// DSL is active and operational
    Active,
    /// DSL is being updated
    Updating,
    /// DSL is suspended
    Suspended,
    /// DSL has failed
    Failed,
    /// DSL is archived
    Archived,
}

/// State change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    /// Event ID
    pub event_id: Uuid,
    /// Instance ID this event relates to
    pub instance_id: Uuid,
    /// Event type
    pub event_type: StateChangeType,
    /// Previous state
    pub previous_state: Option<StateStatus>,
    /// New state
    pub new_state: StateStatus,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// User who triggered the change
    pub user_id: String,
    /// Change description
    pub description: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Types of state changes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum StateChangeType {
    /// State was created
    Created,
    /// State was updated
    Updated,
    /// State was suspended
    Suspended,
    /// State was resumed
    Resumed,
    /// State failed
    Failed,
    /// State was archived
    Archived,
    /// Version was incremented
    VersionIncremented,
}

impl DslStateManager {
    /// Create a new DSL state manager
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            change_history: Vec::new(),
        }
    }

    /// Create a new DSL state
    pub fn create_state(
        &mut self,
        instance_id: Uuid,
        domain: String,
        initial_dsl: String,
        user_id: String,
        metadata: HashMap<String, String>,
    ) -> Result<(), StateManagerError> {
        if self.states.contains_key(&instance_id) {
            return Err(StateManagerError::StateAlreadyExists(instance_id));
        }

        let now = Utc::now();
        let state = DslState {
            instance_id,
            version: 1,
            status: StateStatus::Creating,
            domain,
            current_dsl: initial_dsl,
            created_at: now,
            updated_at: now,
            metadata,
        };

        // Record state creation event
        let event = StateChangeEvent {
            event_id: Uuid::new_v4(),
            instance_id,
            event_type: StateChangeType::Created,
            previous_state: None,
            new_state: StateStatus::Creating,
            timestamp: now,
            user_id,
            description: Some("DSL state created".to_string()),
            metadata: HashMap::new(),
        };

        self.states.insert(instance_id, state);
        self.change_history.push(event);

        Ok(())
    }

    /// Update DSL state
    pub(crate) fn update_state(
        &mut self,
        instance_id: Uuid,
        new_status: StateStatus,
        user_id: String,
        description: Option<String>,
    ) -> Result<(), StateManagerError> {
        let state = self
            .states
            .get_mut(&instance_id)
            .ok_or(StateManagerError::StateNotFound(instance_id))?;

        let previous_status = state.status.clone();
        state.status = new_status.clone();
        state.updated_at = Utc::now();

        // Record state change event
        let event = StateChangeEvent {
            event_id: Uuid::new_v4(),
            instance_id,
            event_type: StateChangeType::Updated,
            previous_state: Some(previous_status),
            new_state: new_status,
            timestamp: Utc::now(),
            user_id,
            description,
            metadata: HashMap::new(),
        };

        self.change_history.push(event);
        Ok(())
    }

    /// Get DSL state
    pub fn get_state(&self, instance_id: &Uuid) -> Option<&DslState> {
        self.states.get(instance_id)
    }

    /// Get state statistics
    pub fn get_statistics(&self) -> StateStatistics {
        let mut stats = StateStatistics::default();
        stats.total_states = self.states.len();

        for state in self.states.values() {
            match state.status {
                StateStatus::Active => stats.active_states += 1,
                StateStatus::Failed => stats.failed_states += 1,
                StateStatus::Archived => stats.archived_states += 1,
                StateStatus::Creating => stats.creating_states += 1,
                StateStatus::Updating => stats.updating_states += 1,
                StateStatus::Suspended => stats.suspended_states += 1,
            }
        }

        stats.total_events = self.change_history.len();
        stats
    }
}

/// State manager error types
#[derive(Debug, thiserror::Error)]
pub(crate) enum StateManagerError {
    #[error("State with ID {0} already exists")]
    StateAlreadyExists(Uuid),

    #[error("State with ID {0} not found")]
    StateNotFound(Uuid),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: StateStatus, to: StateStatus },

    #[error("State is archived and cannot be modified")]
    StateArchived,
}

/// State statistics
#[derive(Debug, Default)]
pub struct StateStatistics {
    pub total_states: usize,
    pub active_states: usize,
    pub failed_states: usize,
    pub archived_states: usize,
    pub creating_states: usize,
    pub updating_states: usize,
    pub suspended_states: usize,
    pub total_events: usize,
}

impl Default for DslStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_state() {
        let mut manager = DslStateManager::new();
        let instance_id = Uuid::new_v4();

        let result = manager.create_state(
            instance_id,
            "test".to_string(),
            "test dsl".to_string(),
            "test_user".to_string(),
            HashMap::new(),
        );

        assert!(result.is_ok());
        let state = manager.get_state(&instance_id).unwrap();
        assert_eq!(state.status, StateStatus::Creating);
        assert_eq!(state.domain, "test");
    }

    #[test]
    fn test_update_state() {
        let mut manager = DslStateManager::new();
        let instance_id = Uuid::new_v4();

        manager
            .create_state(
                instance_id,
                "test".to_string(),
                "test dsl".to_string(),
                "test_user".to_string(),
                HashMap::new(),
            )
            .unwrap();

        let result = manager.update_state(
            instance_id,
            StateStatus::Active,
            "test_user".to_string(),
            None,
        );

        assert!(result.is_ok());
        let state = manager.get_state(&instance_id).unwrap();
        assert_eq!(state.status, StateStatus::Active);
    }

    #[test]
    fn test_state_statistics() {
        let mut manager = DslStateManager::new();

        // Create some test states
        for i in 0..5 {
            let instance_id = Uuid::new_v4();
            manager
                .create_state(
                    instance_id,
                    "test".to_string(),
                    format!("test dsl {}", i),
                    "test_user".to_string(),
                    HashMap::new(),
                )
                .unwrap();

            if i < 3 {
                manager
                    .update_state(
                        instance_id,
                        StateStatus::Active,
                        "test_user".to_string(),
                        None,
                    )
                    .unwrap();
            }
        }

        let stats = manager.get_statistics();
        assert_eq!(stats.total_states, 5);
        assert_eq!(stats.active_states, 3);
        assert_eq!(stats.creating_states, 2);
    }
}
