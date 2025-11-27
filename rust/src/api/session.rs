//! Session state management for Agent API
//!
//! Provides stateful session handling for multi-turn DSL generation conversations.
//! Sessions accumulate intents, validate them, assemble DSL, and track execution.

use super::intent::VerbIntent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ============================================================================
// Session State Machine
// ============================================================================

/// Session lifecycle states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Just created, no intents yet
    New,
    /// Has pending intents awaiting validation
    PendingValidation,
    /// Intents validated, DSL assembled, ready to execute
    ReadyToExecute,
    /// Execution in progress
    Executing,
    /// Execution complete (success or partial)
    Executed,
    /// Session ended
    Closed,
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::New
    }
}

// ============================================================================
// Session Types
// ============================================================================

/// The main agent session - lives server-side
#[derive(Debug, Clone, Serialize)]
pub struct AgentSession {
    /// Unique session identifier
    pub id: Uuid,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Current state in the session lifecycle
    pub state: SessionState,

    /// Conversation history
    pub messages: Vec<ChatMessage>,

    /// Current pending intents (before validation)
    pub pending_intents: Vec<VerbIntent>,

    /// Validated and assembled DSL statements
    pub assembled_dsl: Vec<String>,

    /// Results from execution
    pub executed_results: Vec<ExecutionResult>,

    /// Context accumulated during session
    pub context: SessionContext,
}

impl AgentSession {
    /// Create a new session with optional domain hint
    pub fn new(domain_hint: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            state: SessionState::New,
            messages: Vec::new(),
            pending_intents: Vec::new(),
            assembled_dsl: Vec::new(),
            executed_results: Vec::new(),
            context: SessionContext {
                domain_hint,
                ..Default::default()
            },
        }
    }

    /// Add a user message to the session
    pub fn add_user_message(&mut self, content: String) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::User,
            content,
            timestamp: Utc::now(),
            intents: None,
            dsl: None,
        });
        self.updated_at = Utc::now();
        id
    }

    /// Add an agent message to the session
    pub fn add_agent_message(
        &mut self,
        content: String,
        intents: Option<Vec<VerbIntent>>,
        dsl: Option<String>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.messages.push(ChatMessage {
            id,
            role: MessageRole::Agent,
            content,
            timestamp: Utc::now(),
            intents,
            dsl,
        });
        self.updated_at = Utc::now();
        id
    }

    /// Add intents and transition state
    pub fn add_intents(&mut self, intents: Vec<VerbIntent>) {
        self.pending_intents.extend(intents);
        self.state = SessionState::PendingValidation;
        self.updated_at = Utc::now();
    }

    /// Set assembled DSL after validation (keep intents for execution-time resolution)
    pub fn set_assembled_dsl(&mut self, dsl: Vec<String>) {
        self.assembled_dsl = dsl;
        // NOTE: Don't clear pending_intents - we need them for execution-time ref resolution
        self.state = SessionState::ReadyToExecute;
        self.updated_at = Utc::now();
    }

    /// Clear assembled DSL
    pub fn clear_assembled_dsl(&mut self) {
        self.assembled_dsl.clear();
        self.state = if self.pending_intents.is_empty() {
            SessionState::New
        } else {
            SessionState::PendingValidation
        };
        self.updated_at = Utc::now();
    }

    /// Record execution results and update context
    pub fn record_execution(&mut self, results: Vec<ExecutionResult>) {
        // Update context with created entities
        for result in &results {
            if result.success {
                if let Some(id) = result.entity_id {
                    match result.entity_type.as_deref() {
                        Some("CBU") | Some("cbu") => {
                            self.context.last_cbu_id = Some(id);
                            self.context.cbu_ids.push(id);
                        }
                        Some(_) => {
                            self.context.last_entity_id = Some(id);
                            self.context.entity_ids.push(id);
                        }
                        None => {}
                    }
                }
            }
        }

        self.executed_results = results;
        self.assembled_dsl.clear();
        self.state = SessionState::Executed;
        self.updated_at = Utc::now();
    }

    /// Get all accumulated DSL as a single combined string
    pub fn combined_dsl(&self) -> String {
        self.assembled_dsl.join("\n\n")
    }

    /// Check if the session can execute
    pub fn can_execute(&self) -> bool {
        self.state == SessionState::ReadyToExecute && !self.assembled_dsl.is_empty()
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Unique message ID
    pub id: Uuid,
    /// Who sent this message
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// When the message was sent
    pub timestamp: DateTime<Utc>,
    /// Intents extracted from this message (if user message processed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intents: Option<Vec<VerbIntent>>,
    /// DSL generated from this message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

// ============================================================================
// Session Context
// ============================================================================

/// Context maintained across the session for reference resolution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionContext {
    /// Most recently created CBU
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_cbu_id: Option<Uuid>,
    /// Most recently created entity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_entity_id: Option<Uuid>,
    /// All CBUs created in this session
    #[serde(default)]
    pub cbu_ids: Vec<Uuid>,
    /// All entities created in this session
    #[serde(default)]
    pub entity_ids: Vec<Uuid>,
    /// Domain hint for RAG context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_hint: Option<String>,
    /// Named references for complex workflows
    #[serde(default)]
    pub named_refs: HashMap<String, Uuid>,
}

impl SessionContext {
    /// Resolve a reference like "@last_cbu" or "@last_entity"
    pub fn resolve_ref(&self, ref_name: &str) -> Option<String> {
        match ref_name {
            "@last_cbu" => self.last_cbu_id.map(|u| format!("\"{}\"", u)),
            "@last_entity" => self.last_entity_id.map(|u| format!("\"{}\"", u)),
            _ if ref_name.starts_with('@') => {
                let name = &ref_name[1..];
                self.named_refs.get(name).map(|u| format!("\"{}\"", u))
            }
            _ => None,
        }
    }

    /// Set a named reference
    pub fn set_named_ref(&mut self, name: &str, id: Uuid) {
        self.named_refs.insert(name.to_string(), id);
    }
}

// ============================================================================
// Execution Result
// ============================================================================

/// Result of executing a single DSL statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Index of the statement in the assembled DSL
    pub statement_index: usize,
    /// The DSL statement that was executed
    pub dsl: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Human-readable message about the result
    pub message: String,
    /// Entity ID if one was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Type of entity created (CBU, ENTITY, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
}

// ============================================================================
// Session Store
// ============================================================================

/// Thread-safe in-memory session store
pub type SessionStore = Arc<RwLock<HashMap<Uuid, AgentSession>>>;

/// Create a new session store
pub fn create_session_store() -> SessionStore {
    Arc::new(RwLock::new(HashMap::new()))
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/// Request to create a new session
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    /// Optional domain hint to focus generation
    pub domain_hint: Option<String>,
}

/// Response after creating a session
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    /// The new session ID
    pub session_id: Uuid,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// Initial state
    pub state: SessionState,
}

/// Request to send a chat message
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    /// The user's message
    pub message: String,
}

/// Response from a chat message (intent-based)
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// Agent's response message
    pub message: String,
    /// Extracted intents
    pub intents: Vec<VerbIntent>,
    /// Validation results for each intent
    pub validation_results: Vec<super::intent::IntentValidation>,
    /// Assembled DSL (if all intents valid)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assembled_dsl: Option<super::intent::AssembledDsl>,
    /// Current session state
    pub session_state: SessionState,
    /// Whether the session can execute
    pub can_execute: bool,
}

/// Response with session state
#[derive(Debug, Serialize)]
pub struct SessionStateResponse {
    /// Session ID
    pub session_id: Uuid,
    /// Current state
    pub state: SessionState,
    /// Number of messages in the session
    pub message_count: usize,
    /// Pending intents awaiting validation
    pub pending_intents: Vec<VerbIntent>,
    /// Assembled DSL statements
    pub assembled_dsl: Vec<String>,
    /// Combined DSL
    pub combined_dsl: String,
    /// Session context
    pub context: SessionContext,
    /// Conversation history
    pub messages: Vec<ChatMessage>,
    /// Whether the session can execute
    pub can_execute: bool,
}

/// Request to execute accumulated DSL
#[derive(Debug, Deserialize)]
pub struct ExecuteRequest {
    /// Whether to execute in dry-run mode
    #[serde(default)]
    pub dry_run: bool,
}

/// Response from executing DSL
#[derive(Debug, Serialize)]
pub struct ExecuteResponse {
    /// Overall success status
    pub success: bool,
    /// Results for each DSL statement
    pub results: Vec<ExecutionResult>,
    /// Any errors encountered
    pub errors: Vec<String>,
    /// New session state after execution
    pub new_state: SessionState,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let session = AgentSession::new(Some("cbu".to_string()));
        assert!(!session.id.is_nil());
        assert_eq!(session.state, SessionState::New);
        assert!(session.messages.is_empty());
        assert!(session.assembled_dsl.is_empty());
        assert_eq!(session.context.domain_hint, Some("cbu".to_string()));
    }

    #[test]
    fn test_session_state_transitions() {
        let mut session = AgentSession::new(None);
        assert_eq!(session.state, SessionState::New);

        // Add intents -> PendingValidation
        session.add_intents(vec![VerbIntent {
            verb: "cbu.ensure".to_string(),
            params: Default::default(),
            refs: Default::default(),
            sequence: None,
        }]);
        assert_eq!(session.state, SessionState::PendingValidation);

        // Set assembled DSL -> ReadyToExecute
        session.set_assembled_dsl(vec!["(cbu.ensure :cbu-name \"Test\")".to_string()]);
        assert_eq!(session.state, SessionState::ReadyToExecute);
        assert!(session.pending_intents.is_empty());

        // Record execution -> Executed
        session.record_execution(vec![ExecutionResult {
            statement_index: 0,
            dsl: "(cbu.ensure :cbu-name \"Test\")".to_string(),
            success: true,
            message: "OK".to_string(),
            entity_id: Some(Uuid::new_v4()),
            entity_type: Some("CBU".to_string()),
        }]);
        assert_eq!(session.state, SessionState::Executed);
        assert!(session.assembled_dsl.is_empty());
    }

    #[test]
    fn test_context_resolve_ref() {
        let mut ctx = SessionContext::default();
        let cbu_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();

        ctx.last_cbu_id = Some(cbu_id);
        ctx.last_entity_id = Some(entity_id);

        assert_eq!(
            ctx.resolve_ref("@last_cbu"),
            Some(format!("\"{}\"", cbu_id))
        );
        assert_eq!(
            ctx.resolve_ref("@last_entity"),
            Some(format!("\"{}\"", entity_id))
        );
        assert_eq!(ctx.resolve_ref("@unknown"), None);
    }

    #[test]
    fn test_context_named_refs() {
        let mut ctx = SessionContext::default();
        let id = Uuid::new_v4();

        ctx.set_named_ref("my_entity", id);
        assert_eq!(ctx.resolve_ref("@my_entity"), Some(format!("\"{}\"", id)));
    }

    #[test]
    fn test_add_messages() {
        let mut session = AgentSession::new(None);

        let user_id = session.add_user_message("Create a CBU".to_string());
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].id, user_id);
        assert_eq!(session.messages[0].role, MessageRole::User);

        let agent_id = session.add_agent_message(
            "Here's the DSL".to_string(),
            None,
            Some("(cbu.ensure :cbu-name \"Test\")".to_string()),
        );
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].id, agent_id);
        assert_eq!(session.messages[1].role, MessageRole::Agent);
        assert!(session.messages[1].dsl.is_some());
    }

    #[tokio::test]
    async fn test_session_store() {
        let store = create_session_store();
        let session = AgentSession::new(None);
        let id = session.id;

        // Insert
        {
            let mut write = store.write().await;
            write.insert(id, session);
        }

        // Read
        {
            let read = store.read().await;
            assert!(read.contains_key(&id));
            assert_eq!(read.get(&id).unwrap().state, SessionState::New);
        }
    }
}
