//! Runtime Environment for the DSL Forth Engine.
//!
//! Provides database-backed storage for attributes and documents during DSL execution.

use crate::cbu_model_dsl::ast::CbuModel;
use crate::forth_engine::value::CrudStatement;
use crate::forth_engine::value::{AttributeId, DocumentId, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnboardingRequestId(pub String);

/// The RuntimeEnv holds state for a single DSL execution session.
/// It provides access to the database for reading/writing attributes and documents.
pub struct RuntimeEnv {
    /// The case/request ID for this execution
    pub request_id: OnboardingRequestId,

    /// Database connection pool (when database feature enabled)
    #[cfg(feature = "database")]
    pub pool: Option<PgPool>,

    /// Current CBU ID for this execution context
    pub cbu_id: Option<Uuid>,

    /// Current entity ID for this execution context
    pub entity_id: Option<Uuid>,

    /// Current investigation ID for this execution context
    pub investigation_id: Option<Uuid>,

    /// Current decision ID for this execution context
    pub decision_id: Option<Uuid>,

    /// In-memory cache for attributes during execution
    pub attribute_cache: HashMap<AttributeId, Value>,

    /// In-memory cache for documents during execution
    pub document_cache: HashMap<DocumentId, DocumentMeta>,

    /// Extracted case_id from DSL execution
    pub case_id: Option<String>,

    /// Sink attributes - attributes that should be populated for this context
    pub sink_attributes: HashSet<Uuid>,

    /// Source attributes - attributes that produce data in this context
    pub source_attributes: HashSet<Uuid>,

    /// CBU Model specification for validation (Phase 5)
    pub cbu_model: Option<CbuModel>,

    /// CBU Model ID for tracking which model is in use
    pub cbu_model_id: Option<String>,

    /// Pending CRUD statements to be executed (Phase 6)
    pub pending_crud: Vec<CrudStatement>,

    /// Current CBU state for state machine validation
    pub cbu_state: Option<String>,

    /// Current transition verb being executed
    pub current_transition_verb: Option<String>,

    /// Current chunks being processed
    pub current_chunks: Vec<String>,
}

/// Document metadata
#[derive(Debug, Clone)]
pub struct DocumentMeta {
    pub id: DocumentId,
    pub name: String,
    pub doc_type: String,
    pub location: Option<String>,
}

impl RuntimeEnv {
    /// Create a new RuntimeEnv without database connection
    pub fn new(request_id: OnboardingRequestId) -> Self {
        Self {
            request_id,
            #[cfg(feature = "database")]
            pool: None,
            cbu_id: None,
            entity_id: None,
            investigation_id: None,
            decision_id: None,
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
            sink_attributes: HashSet::new(),
            source_attributes: HashSet::new(),
            cbu_model: None,
            cbu_model_id: None,
            pending_crud: Vec::new(),
            cbu_state: None,
            current_transition_verb: None,
            current_chunks: Vec::new(),
        }
    }

    /// Create a new RuntimeEnv with database connection
    #[cfg(feature = "database")]
    pub fn with_pool(request_id: OnboardingRequestId, pool: PgPool) -> Self {
        Self {
            request_id,
            pool: Some(pool),
            cbu_id: None,
            entity_id: None,
            investigation_id: None,
            decision_id: None,
            attribute_cache: HashMap::new(),
            document_cache: HashMap::new(),
            case_id: None,
            sink_attributes: HashSet::new(),
            source_attributes: HashSet::new(),
            cbu_model: None,
            cbu_model_id: None,
            pending_crud: Vec::new(),
            cbu_state: None,
            current_transition_verb: None,
            current_chunks: Vec::new(),
        }
    }

    /// Set the CBU ID for this execution context
    pub fn set_cbu_id(&mut self, id: Uuid) {
        self.cbu_id = Some(id);
    }

    /// Get the CBU ID, returning error if not set
    pub fn ensure_cbu_id(&self) -> Result<Uuid, &'static str> {
        self.cbu_id.ok_or("CBU ID not set in runtime environment")
    }

    /// Set the entity ID for this execution context
    pub fn set_entity_id(&mut self, id: Uuid) {
        self.entity_id = Some(id);
    }

    /// Get the entity ID, returning error if not set
    pub fn ensure_entity_id(&self) -> Result<Uuid, &'static str> {
        self.entity_id
            .ok_or("Entity ID not set in runtime environment")
    }

    /// Set the investigation ID for this execution context
    pub fn set_investigation_id(&mut self, id: Uuid) {
        self.investigation_id = Some(id);
    }

    /// Get the investigation ID, returning error if not set
    pub fn ensure_investigation_id(&self) -> Result<Uuid, &'static str> {
        self.investigation_id
            .ok_or("Investigation ID not set in runtime environment")
    }

    /// Set the decision ID for this execution context
    pub fn set_decision_id(&mut self, id: Uuid) {
        self.decision_id = Some(id);
    }

    /// Get the decision ID, returning error if not set
    pub fn ensure_decision_id(&self) -> Result<Uuid, &'static str> {
        self.decision_id
            .ok_or("Decision ID not set in runtime environment")
    }

    /// Check if an attribute is a sink for this context
    pub fn is_sink(&self, attr_id: &Uuid) -> bool {
        self.sink_attributes.contains(attr_id)
    }

    /// Check if an attribute is a source for this context
    pub fn is_source(&self, attr_id: &Uuid) -> bool {
        self.source_attributes.contains(attr_id)
    }

    /// Add a sink attribute
    pub fn add_sink_attribute(&mut self, attr_id: Uuid) {
        self.sink_attributes.insert(attr_id);
    }

    /// Add a source attribute
    pub fn add_source_attribute(&mut self, attr_id: Uuid) {
        self.source_attributes.insert(attr_id);
    }

    /// Set sink attributes from a list
    pub fn set_sink_attributes(&mut self, attrs: Vec<Uuid>) {
        self.sink_attributes = attrs.into_iter().collect();
    }

    /// Set source attributes from a list
    pub fn set_source_attributes(&mut self, attrs: Vec<Uuid>) {
        self.source_attributes = attrs.into_iter().collect();
    }

    /// Check if database is available
    #[cfg(feature = "database")]
    pub fn has_database(&self) -> bool {
        self.pool.is_some()
    }

    #[cfg(not(feature = "database"))]
    pub fn has_database(&self) -> bool {
        false
    }

    /// Set the CBU Model for validation
    pub fn set_cbu_model(&mut self, model: CbuModel) {
        // Set initial state from model
        self.cbu_state = Some(model.states.initial.clone());
        self.cbu_model_id = Some(model.id.clone());
        self.cbu_model = Some(model);
    }

    /// Get the CBU Model ID
    pub fn get_cbu_model_id(&self) -> Option<&str> {
        self.cbu_model_id.as_deref()
    }

    /// Set the current transition verb being executed
    pub fn set_current_transition(&mut self, verb: &str) {
        self.current_transition_verb = Some(verb.to_string());

        // Look up chunks for this transition from the model
        if let Some(model) = &self.cbu_model {
            if let Some(transition) = model.find_transition_by_verb(verb) {
                self.current_chunks = transition.chunks.clone();
            }
        }
    }

    /// Get the current transition verb
    pub fn get_current_transition(&self) -> Option<&str> {
        self.current_transition_verb.as_deref()
    }

    /// Get the current chunks being processed
    pub fn get_current_chunks(&self) -> &[String] {
        &self.current_chunks
    }

    /// Get the CBU Model
    pub fn get_cbu_model(&self) -> Option<&CbuModel> {
        self.cbu_model.as_ref()
    }

    /// Add a CRUD statement to pending operations
    pub fn push_crud(&mut self, stmt: CrudStatement) {
        self.pending_crud.push(stmt);
    }

    /// Get pending CRUD statements
    pub fn get_pending_crud(&self) -> &[CrudStatement] {
        &self.pending_crud
    }

    /// Take pending CRUD statements (drains the list)
    pub fn take_pending_crud(&mut self) -> Vec<CrudStatement> {
        std::mem::take(&mut self.pending_crud)
    }

    /// Set current CBU state
    pub fn set_cbu_state(&mut self, state: String) {
        self.cbu_state = Some(state);
    }

    /// Get current CBU state
    pub fn get_cbu_state(&self) -> Option<&str> {
        self.cbu_state.as_deref()
    }

    /// Check if a state transition is valid according to the CBU Model
    pub fn is_valid_transition(&self, to_state: &str) -> bool {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => {
                model.states.is_valid_transition(from_state, to_state)
            }
            _ => true, // No model or state = no validation
        }
    }

    /// Get the verb required for a state transition
    pub fn get_transition_verb(&self, to_state: &str) -> Option<String> {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => model
                .states
                .get_transition(from_state, to_state)
                .map(|t| t.verb.clone()),
            _ => None,
        }
    }

    /// Check if all required attributes are present for a transition
    pub fn check_transition_preconditions(&self, to_state: &str) -> Vec<String> {
        match (&self.cbu_model, &self.cbu_state) {
            (Some(model), Some(from_state)) => {
                if let Some(transition) = model.states.get_transition(from_state, to_state) {
                    let present: Vec<&str> =
                        self.attribute_cache.keys().map(|k| k.0.as_str()).collect();
                    transition
                        .check_preconditions(&present)
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    /// Get attribute from cache (sync - for VM execution)
    pub fn get_attribute(&self, id: &AttributeId) -> Option<&Value> {
        self.attribute_cache.get(id)
    }

    /// Set attribute in cache (will be persisted at end of execution)
    pub fn set_attribute(&mut self, id: AttributeId, value: Value) {
        self.attribute_cache.insert(id, value);
    }

    /// Set the case_id extracted during execution
    pub fn set_case_id(&mut self, case_id: String) {
        self.case_id = Some(case_id);
    }

    /// Get the case_id
    pub fn get_case_id(&self) -> Option<&String> {
        self.case_id.as_ref()
    }

    // Note: DB operations for DSL/AST persistence, CBU creation, and attribute saving
    // have been moved to the central database facade (DslRepository).
    // RuntimeEnv now only handles in-memory caching and attribute loading during execution.
    // See crate::database::DslRepository for transactional DSL/AST saves.
    // Attribute loading from DB should use AttributeValuesService via CrudExecutor.
}

/// Generate a new OB Request ID
pub fn mint_ob_request_id() -> String {
    let uuid = uuid::Uuid::new_v4();
    format!("OB-{}", &uuid.to_string()[..8].to_uppercase())
}

/// Generate DSL onboarding template with minted ID
pub fn generate_onboarding_template(
    ob_request_id: &str,
    client_name: &str,
    client_type: &str,
) -> String {
    format!(
        r#"(case.create :case-id "{}" :case-type "ONBOARDING" :client-name "{}" :client-type "{}")"#,
        ob_request_id, client_name, client_type
    )
}
