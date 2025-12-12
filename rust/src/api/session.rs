//! Session state management for Agent API
//!
//! Provides stateful session handling for multi-turn DSL generation conversations.
//! Sessions accumulate AST statements, validate them, and track execution.
//! The AST is the source of truth - DSL source is generated from it for display.

use super::intent::VerbIntent;
use crate::dsl_v2::ast::{Program, Statement};
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
#[derive(Default)]
pub enum SessionState {
    /// Just created, no intents yet
    #[default]
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

/// Status of a DSL/AST pair in the pipeline
/// This is the state machine for individual DSL fragments
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DslStatus {
    /// DSL parsed and AST valid, awaiting user confirmation
    #[default]
    Draft,
    /// User confirmed, ready to execute
    Ready,
    /// Successfully executed against database
    Executed,
    /// User declined to run (logical delete)
    Cancelled,
    /// Execution attempted but failed
    Failed,
}

impl DslStatus {
    /// Can this DSL be executed?
    pub fn is_runnable(&self) -> bool {
        matches!(self, DslStatus::Draft | DslStatus::Ready)
    }

    /// Is this DSL in a terminal state?
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DslStatus::Executed | DslStatus::Cancelled | DslStatus::Failed
        )
    }

    /// Should this DSL be persisted to database?
    pub fn should_persist(&self) -> bool {
        // Only persist executed DSL - drafts and cancelled stay in memory
        matches!(self, DslStatus::Executed)
    }
}

/// Pending DSL/AST pair in the session (not yet persisted to DB)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDsl {
    /// Unique ID for this pending DSL fragment
    pub id: Uuid,
    /// The DSL source code
    pub source: String,
    /// Parsed AST statements
    pub ast: Vec<Statement>,
    /// Current status
    pub status: DslStatus,
    /// When this was created
    pub created_at: DateTime<Utc>,
    /// Error message if status is Failed
    pub error: Option<String>,
    /// Bindings that would be created if executed
    pub pending_bindings: HashMap<String, String>,
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

    /// Validated and assembled DSL statements (legacy - for backward compat)
    pub assembled_dsl: Vec<String>,

    /// Current pending DSL/AST awaiting user confirmation (in-memory only)
    /// This is cleared on execute, cancel, or new chat message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingDsl>,

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
            pending: None,
            executed_results: Vec::new(),
            context: SessionContext {
                domain_hint,
                ..Default::default()
            },
        }
    }

    /// Set pending DSL (parsed and ready for user confirmation)
    pub fn set_pending_dsl(&mut self, source: String, ast: Vec<Statement>) {
        let pending_bindings = ast
            .iter()
            .filter_map(|stmt| {
                if let Statement::VerbCall(vc) = stmt {
                    vc.binding.as_ref().map(|b| (b.clone(), vc.domain.clone()))
                } else {
                    None
                }
            })
            .collect();

        self.pending = Some(PendingDsl {
            id: Uuid::new_v4(),
            source,
            ast,
            status: DslStatus::Draft,
            created_at: Utc::now(),
            error: None,
            pending_bindings,
        });
        self.state = SessionState::ReadyToExecute;
        self.updated_at = Utc::now();
    }

    /// Cancel pending DSL (user declined)
    pub fn cancel_pending(&mut self) {
        if let Some(ref mut pending) = self.pending {
            pending.status = DslStatus::Cancelled;
        }
        // Clear pending - cancelled drafts don't persist
        self.pending = None;
        self.state = SessionState::Executed; // Ready for next command
        self.updated_at = Utc::now();
    }

    /// Mark pending DSL as ready to execute (user confirmed)
    pub fn confirm_pending(&mut self) {
        if let Some(ref mut pending) = self.pending {
            pending.status = DslStatus::Ready;
        }
        self.updated_at = Utc::now();
    }

    /// Mark pending DSL as executed (after successful execution)
    pub fn mark_executed(&mut self) {
        if let Some(ref mut pending) = self.pending {
            pending.status = DslStatus::Executed;
        }
        self.state = SessionState::Executed;
        self.updated_at = Utc::now();
    }

    /// Mark pending DSL as failed (execution error)
    pub fn mark_failed(&mut self, error: String) {
        if let Some(ref mut pending) = self.pending {
            pending.status = DslStatus::Failed;
            pending.error = Some(error);
        }
        self.state = SessionState::Executed; // Can try again
        self.updated_at = Utc::now();
    }

    /// Get pending DSL if in runnable state
    pub fn get_runnable_dsl(&self) -> Option<&PendingDsl> {
        self.pending.as_ref().filter(|p| p.status.is_runnable())
    }

    /// Check if there's pending DSL awaiting confirmation
    pub fn has_pending(&self) -> bool {
        self.pending
            .as_ref()
            .map(|p| !p.status.is_terminal())
            .unwrap_or(false)
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

/// Information about a bound entity in the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundEntity {
    /// The UUID of the entity
    pub id: Uuid,
    /// The entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name (e.g., "Aviva Lux 9")
    pub display_name: String,
}

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
    /// Named references for complex workflows (legacy - UUID only)
    #[serde(default)]
    pub named_refs: HashMap<String, Uuid>,
    /// Typed bindings with display names for LLM context (populated after execution)
    #[serde(default)]
    pub bindings: HashMap<String, BoundEntity>,
    /// Pending bindings from assembled DSL that hasn't been executed yet
    /// Format: binding_name -> (inferred_type, display_name)
    /// These are extracted from :as @name patterns in DSL
    #[serde(default)]
    pub pending_bindings: HashMap<String, (String, String)>,
    /// The accumulated AST - source of truth for the session's DSL
    /// Each chat message can add/modify statements in this AST
    #[serde(default)]
    pub ast: Vec<Statement>,
    /// Index from binding name to AST statement index
    /// Allows lookup like: get_ast_by_key("cbu_id") → returns the Statement that created it
    #[serde(default)]
    pub ast_index: HashMap<String, usize>,
    /// The ACTIVE CBU for this session - used as implicit context for incremental operations
    /// When set, operations like cbu.add-product will auto-use this CBU ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_cbu: Option<BoundEntity>,
    /// Primary domain keys - the main identifiers for this onboarding session
    #[serde(default)]
    pub primary_keys: PrimaryDomainKeys,
}

/// Primary domain keys tracked across the session
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrimaryDomainKeys {
    /// Onboarding request ID (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding_request_id: Option<Uuid>,
    /// Primary CBU being onboarded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbu_id: Option<Uuid>,
    /// Primary KYC case for this onboarding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kyc_case_id: Option<Uuid>,
    /// Primary document collection (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_batch_id: Option<Uuid>,
    /// Primary service resource instance
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_instance_id: Option<Uuid>,
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

    /// Set a typed binding with display name
    /// Returns the actual binding name used (may have suffix if collision)
    pub fn set_binding(
        &mut self,
        name: &str,
        id: Uuid,
        entity_type: &str,
        display_name: &str,
    ) -> String {
        // Handle collision - append suffix if name already exists
        let actual_name = if self.bindings.contains_key(name) {
            // Find unique name with suffix
            let mut suffix = 2;
            loop {
                let candidate = format!("{}_{}", name, suffix);
                if !self.bindings.contains_key(&candidate) {
                    break candidate;
                }
                suffix += 1;
            }
        } else {
            name.to_string()
        };

        // Also set in named_refs for backward compatibility
        self.named_refs.insert(actual_name.clone(), id);
        self.bindings.insert(
            actual_name.clone(),
            BoundEntity {
                id,
                entity_type: entity_type.to_string(),
                display_name: display_name.to_string(),
            },
        );

        actual_name
    }

    /// Get bindings formatted for LLM context
    /// Returns strings like "@aviva_lux_9 (CBU: Aviva Lux 9)"
    pub fn bindings_for_llm(&self) -> Vec<String> {
        self.bindings
            .iter()
            .map(|(name, binding)| {
                format!(
                    "@{} ({}: {})",
                    name,
                    binding.entity_type.to_uppercase(),
                    binding.display_name
                )
            })
            .collect()
    }

    /// Get the active CBU context formatted for LLM
    /// Returns something like "ACTIVE_CBU: Aviva Lux 9 (uuid: 327804f8-...)"
    pub fn active_cbu_for_llm(&self) -> Option<String> {
        self.active_cbu
            .as_ref()
            .map(|cbu| format!("ACTIVE_CBU: \"{}\" (id: {})", cbu.display_name, cbu.id))
    }

    /// Set the active CBU for this session
    pub fn set_active_cbu(&mut self, id: Uuid, display_name: &str) {
        self.active_cbu = Some(BoundEntity {
            id,
            entity_type: "cbu".to_string(),
            display_name: display_name.to_string(),
        });
    }

    /// Clear the active CBU
    pub fn clear_active_cbu(&mut self) {
        self.active_cbu = None;
    }

    // =========================================================================
    // AST MANIPULATION
    // =========================================================================

    /// Add statements to the AST
    pub fn add_statements(&mut self, statements: Vec<Statement>) {
        for stmt in statements {
            self.add_statement(stmt);
        }
    }

    /// Add a single statement to the AST, indexing by binding name if present
    pub fn add_statement(&mut self, statement: Statement) {
        let idx = self.ast.len();

        // If statement has a binding (:as @name), index it
        if let Statement::VerbCall(ref verb_call) = statement {
            if let Some(ref binding_name) = verb_call.binding {
                self.ast_index.insert(binding_name.to_string(), idx);

                // Also update primary keys based on domain
                let domain = &verb_call.domain;
                if domain == "cbu" && self.primary_keys.cbu_id.is_none() {
                    // Will be set when we get the UUID from execution
                }
                if domain == "kyc-case" && self.primary_keys.kyc_case_id.is_none() {
                    // Will be set when we get the UUID from execution
                }
            }
        }

        self.ast.push(statement);
    }

    /// Get AST statement by binding key (e.g., "cbu_id" → the cbu.ensure statement)
    pub fn get_ast_by_key(&self, key: &str) -> Option<&Statement> {
        self.ast_index.get(key).and_then(|&idx| self.ast.get(idx))
    }

    /// Get AST statement index by binding key
    pub fn get_ast_index_by_key(&self, key: &str) -> Option<usize> {
        self.ast_index.get(key).copied()
    }

    /// Update primary keys from execution result
    pub fn update_primary_key(&mut self, domain: &str, binding: &str, id: Uuid) {
        match domain {
            "cbu" => {
                if self.primary_keys.cbu_id.is_none() {
                    self.primary_keys.cbu_id = Some(id);
                }
            }
            "kyc-case" => {
                if self.primary_keys.kyc_case_id.is_none() {
                    self.primary_keys.kyc_case_id = Some(id);
                }
            }
            "service-resource" => {
                if self.primary_keys.resource_instance_id.is_none() {
                    self.primary_keys.resource_instance_id = Some(id);
                }
            }
            _ => {}
        }
        // Also index by the specific binding name
        if let Some(idx) = self.ast_index.get(binding) {
            // Already indexed when statement was added
            let _ = idx;
        }
    }

    /// Get the AST as a Program for compilation/execution
    pub fn as_program(&self) -> Program {
        Program {
            statements: self.ast.clone(),
        }
    }

    /// Render the AST back to DSL source for display
    pub fn to_dsl_source(&self) -> String {
        self.ast
            .iter()
            .map(|s| s.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Find a statement by binding name
    pub fn find_by_binding(&self, binding_name: &str) -> Option<&Statement> {
        self.ast.iter().find(|s| {
            if let Statement::VerbCall(vc) = s {
                vc.binding.as_deref() == Some(binding_name)
            } else {
                false
            }
        })
    }

    /// Update a statement's argument value by binding name
    pub fn update_arg(
        &mut self,
        binding_name: &str,
        arg_key: &str,
        new_value: crate::dsl_v2::ast::AstNode,
    ) -> bool {
        for stmt in &mut self.ast {
            if let Statement::VerbCall(vc) = stmt {
                if vc.binding.as_deref() == Some(binding_name) {
                    for arg in &mut vc.arguments {
                        if arg.key == arg_key {
                            arg.value = new_value;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Remove a statement by binding name
    pub fn remove_by_binding(&mut self, binding_name: &str) -> bool {
        let original_len = self.ast.len();
        self.ast.retain(|s| {
            if let Statement::VerbCall(vc) = s {
                vc.binding.as_deref() != Some(binding_name)
            } else {
                true
            }
        });
        self.ast.len() != original_len
    }

    /// Clear all AST statements
    pub fn clear_ast(&mut self) {
        self.ast.clear();
    }

    /// Get count of statements
    pub fn statement_count(&self) -> usize {
        self.ast.len()
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
    /// Result data for Record/RecordSet operations (e.g., cbu.show)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
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
    /// DSL source rendered from AST (for display in UI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl_source: Option<String>,
    /// The full AST for debugging (JSON serialized)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ast: Option<Vec<Statement>>,
    /// Session bindings with type info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<HashMap<String, BoundEntity>>,
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
    /// All bindings created during execution (name -> UUID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bindings: Option<std::collections::HashMap<String, uuid::Uuid>>,
}

// ============================================================================
// Disambiguation Types
// ============================================================================

/// Disambiguation request - sent when entity references are ambiguous
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationRequest {
    /// Unique ID for this disambiguation request
    pub request_id: Uuid,
    /// The ambiguous items that need resolution
    pub items: Vec<DisambiguationItem>,
    /// Human-readable prompt for the user
    pub prompt: String,
    /// Original intents that need disambiguation (preserved for re-processing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_intents: Option<Vec<VerbIntent>>,
}

/// A single ambiguous item needing resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationItem {
    /// Multiple entities match a search term
    EntityMatch {
        /// Parameter name (e.g., "entity-id")
        param: String,
        /// Original search text (e.g., "John Smith")
        search_text: String,
        /// Matching entities to choose from
        matches: Vec<EntityMatchOption>,
    },
    /// Ambiguous interpretation (e.g., "UK" = name part or jurisdiction?)
    InterpretationChoice {
        /// The ambiguous text
        text: String,
        /// Possible interpretations
        options: Vec<Interpretation>,
    },
}

/// A matching entity for disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMatchOption {
    /// Entity UUID
    pub entity_id: Uuid,
    /// Display name
    pub name: String,
    /// Entity type (e.g., "proper_person", "limited_company")
    pub entity_type: String,
    /// Jurisdiction code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jurisdiction: Option<String>,
    /// Additional context (roles, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    /// Match score (0.0 - 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

/// A possible interpretation of ambiguous text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    /// Interpretation ID
    pub id: String,
    /// Human-readable label
    pub label: String,
    /// What this interpretation means
    pub description: String,
    /// How this affects the generated DSL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
}

/// User's disambiguation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisambiguationResponse {
    /// The request ID being responded to
    pub request_id: Uuid,
    /// Selected resolutions
    pub selections: Vec<DisambiguationSelection>,
}

/// A single disambiguation selection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DisambiguationSelection {
    /// Selected entity for an EntityMatch
    Entity { param: String, entity_id: Uuid },
    /// Selected interpretation for an InterpretationChoice
    Interpretation {
        text: String,
        interpretation_id: String,
    },
}

/// Chat response status - indicates whether response is ready or needs disambiguation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ChatResponseStatus {
    /// DSL is ready (no ambiguity or already resolved)
    Ready,
    /// Needs user disambiguation before generating DSL
    NeedsDisambiguation {
        disambiguation: DisambiguationRequest,
    },
    /// Error occurred
    Error { message: String },
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
            lookups: None,
            sequence: None,
        }]);
        assert_eq!(session.state, SessionState::PendingValidation);

        // Set assembled DSL -> ReadyToExecute
        // NOTE: pending_intents are kept for execution-time ref resolution
        session.set_assembled_dsl(vec!["(cbu.ensure :cbu-name \"Test\")".to_string()]);
        assert_eq!(session.state, SessionState::ReadyToExecute);
        assert_eq!(session.pending_intents.len(), 1); // Intents preserved

        // Record execution -> Executed
        session.record_execution(vec![ExecutionResult {
            statement_index: 0,
            dsl: "(cbu.ensure :cbu-name \"Test\")".to_string(),
            success: true,
            message: "OK".to_string(),
            entity_id: Some(Uuid::new_v4()),
            entity_type: Some("CBU".to_string()),
            result: None,
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
