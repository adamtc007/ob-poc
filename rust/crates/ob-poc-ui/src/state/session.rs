//! Session State Management
//!
//! Tracks the current agent session, CBU context, and accumulated bindings.

use std::collections::HashMap;
use uuid::Uuid;

/// Session context - tracks the current working context across chat interactions
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    /// Current session ID (from Rust backend)
    pub session_id: Option<Uuid>,
    /// Currently selected CBU
    pub cbu: Option<CbuContext>,
    /// Current KYC case (if any)
    pub case: Option<CaseContext>,
    /// Named bindings from DSL execution (@symbol -> entity info)
    pub bindings: HashMap<String, BoundEntity>,
}

/// CBU context for display and API calls
#[derive(Debug, Clone)]
pub struct CbuContext {
    pub id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
}

/// Case context for KYC workflow
#[derive(Debug, Clone)]
pub struct CaseContext {
    pub id: Uuid,
    pub case_type: String,
    pub status: Option<String>,
}

/// Bound entity from DSL execution
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BoundEntity {
    pub id: String,
    pub entity_type: String,
    pub display_name: String,
}

impl SessionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current CBU context
    pub fn set_cbu(&mut self, cbu: CbuContext) {
        // Add default bindings for CBU
        self.bindings.insert(
            "cbu".to_string(),
            BoundEntity {
                id: cbu.id.to_string(),
                entity_type: "cbu".to_string(),
                display_name: cbu.name.clone(),
            },
        );
        self.bindings.insert(
            "cbu_id".to_string(),
            BoundEntity {
                id: cbu.id.to_string(),
                entity_type: "cbu".to_string(),
                display_name: cbu.name.clone(),
            },
        );
        self.cbu = Some(cbu);
    }

    /// Clear the session and all context
    pub fn clear(&mut self) {
        self.session_id = None;
        self.cbu = None;
        self.case = None;
        self.bindings.clear();
    }

    /// Update bindings from execution response
    pub fn update_bindings(&mut self, bindings: HashMap<String, BoundEntity>) {
        self.bindings.extend(bindings);
    }

    /// Get binding by name
    pub fn get_binding(&self, name: &str) -> Option<&BoundEntity> {
        self.bindings.get(name)
    }

    /// Check if session is active
    pub fn has_session(&self) -> bool {
        self.session_id.is_some()
    }

    /// Check if CBU is selected
    pub fn has_cbu(&self) -> bool {
        self.cbu.is_some()
    }
}

/// Chat message in the conversation
#[derive(Debug, Clone)]
pub enum ChatMessage {
    /// User message
    User { text: String },
    /// Assistant response with optional DSL
    Assistant {
        text: String,
        dsl: Option<String>,
        status: MessageStatus,
    },
    /// System message (info, warnings)
    System { text: String, level: SystemLevel },
    /// Execution result
    ExecutionResult {
        success: bool,
        message: String,
        created_entities: Vec<CreatedEntity>,
    },
}

/// Status of an assistant message
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageStatus {
    /// Message received, no DSL
    Info,
    /// DSL generated and valid
    Valid,
    /// DSL has validation errors
    Error,
    /// DSL needs user confirmation
    PendingConfirmation,
    /// DSL was executed successfully
    Executed,
}

/// System message level
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemLevel {
    Info,
    Warning,
    Error,
}

/// Entity created during execution
#[derive(Debug, Clone)]
pub struct CreatedEntity {
    pub binding: String,
    pub entity_type: String,
    pub name: String,
    pub id: Uuid,
}

/// Pending state for confirmation flows
#[derive(Debug, Clone, Default)]
pub struct PendingState {
    /// DSL awaiting confirmation
    pub pending_dsl: Option<String>,
    /// Validation corrections to choose from
    pub pending_corrections: Vec<ValidationCorrection>,
    /// Entity selection for disambiguation
    pub pending_entity_selection: Option<EntitySelectionState>,
}

/// Validation correction suggestion
#[derive(Debug, Clone)]
pub struct ValidationCorrection {
    pub correction_type: CorrectionType,
    pub line: usize,
    pub current: String,
    pub suggested: String,
    pub confidence: f32,
    pub available: Vec<String>,
}

/// Type of correction
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrectionType {
    Lookup,
    Verb,
}

/// State for entity disambiguation flow
#[derive(Debug, Clone)]
pub struct EntitySelectionState {
    pub original_message: String,
    pub search_query: String,
    pub results: Vec<EntitySearchResult>,
    pub create_option: String,
}

/// Entity search result
#[derive(Debug, Clone, serde::Deserialize)]
pub struct EntitySearchResult {
    pub entity_id: String,
    pub name: String,
    pub entity_type: String,
    #[serde(default)]
    pub entity_type_code: Option<String>,
    #[serde(default)]
    pub jurisdiction: Option<String>,
    #[serde(default)]
    pub similarity: f32,
}

impl PendingState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any confirmation is pending
    pub fn has_pending(&self) -> bool {
        self.pending_dsl.is_some()
            || !self.pending_corrections.is_empty()
            || self.pending_entity_selection.is_some()
    }

    /// Clear all pending state
    pub fn clear(&mut self) {
        self.pending_dsl = None;
        self.pending_corrections.clear();
        self.pending_entity_selection = None;
    }
}
