//! Session state management for Agent API
//!
//! Provides stateful session handling for multi-turn DSL generation conversations.
//! Sessions accumulate AST statements, validate them, and track execution.
//! The AST is the source of truth - DSL source is generated from it for display.

use crate::dsl_v2::ast::{Program, Statement};
use crate::mcp::scope_resolution::ScopeContext;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ============================================================================
// Session Mode - What kind of interaction is happening
// ============================================================================

/// Session mode - determines how the session processes user input
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SessionMode {
    /// Normal chat mode - agent generates DSL from natural language
    #[default]
    Chat,
    /// Template expansion mode - agent is collecting params for a template
    TemplateExpansion,
    /// Batch execution mode - iterating over a key set, expanding template per item
    BatchExecution,
}

// ============================================================================
// Session State Machine
// ============================================================================

/// Session lifecycle states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub(crate) enum SessionState {
    /// Just created, awaiting scope selection (client/CBU set)
    /// This is the initial state - nothing else can happen until scope is set
    #[default]
    New,
    /// Scope is set (client/CBU set selected), ready for operations
    Scoped,
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

// ============================================================================
// Message Types
// ============================================================================

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatMessage {
    /// Unique message ID
    pub id: Uuid,
    /// Who sent this message
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// When the message was sent
    pub timestamp: DateTime<Utc>,
    /// Intents extracted from this message (legacy, always None)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intents: Option<serde_json::Value>,
    /// DSL generated from this message (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsl: Option<String>,
    /// Sage explanation payload for this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sage_explain: Option<ob_poc_types::chat::SageExplainPayload>,
    /// Drafter proposal payload for this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drafter_proposal: Option<ob_poc_types::chat::DraftProposalPayload>,
    /// Sem OS discovery bootstrap payload for this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_bootstrap: Option<ob_poc_types::chat::DiscoveryBootstrapPayload>,
    /// Parked runbook payload for this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parked_entries: Option<Vec<ob_poc_types::chat::ParkedEntryPayload>>,
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum MessageRole {
    User,
    Agent,
    System,
}

// ============================================================================
// Session Context
// ============================================================================

/// Information about a bound entity in the session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BoundEntity {
    /// The UUID of the entity
    pub id: Uuid,
    /// The entity type (e.g., "cbu", "entity", "case")
    pub entity_type: String,
    /// Human-readable display name (e.g., "Aviva Lux 9")
    pub display_name: String,
}

// ============================================================================
// Batch Context - For bulk REPL operations
// ============================================================================

// ============================================================================
// Progress Structs (replacing anonymous tuples for function returns)
// ============================================================================

/// Status of a batch item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BatchItemStatus {
    /// Not yet processed
    #[default]
    Pending,
    /// Currently being processed (DSL in editor)
    Active,
    /// Successfully executed
    Completed,
    /// Skipped by user
    Skipped,
    /// Execution failed
    Failed,
}

/// A single item in the batch working set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BatchItem {
    /// Source entity ID (e.g., fund entity that will become a CBU)
    pub source_id: Uuid,
    /// Display name for the item
    pub name: String,
    /// Source entity type (e.g., "fund", "entity")
    pub source_type: String,
    /// Additional metadata for display/context
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Processing status
    #[serde(default)]
    pub status: BatchItemStatus,
    /// Created entity ID after execution (e.g., the CBU ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_id: Option<Uuid>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Template Key Set - Collected entity references for template expansion
// ============================================================================

/// A resolved entity reference for template expansion
/// This is the LookupRef triplet: (entity_type, search_key, uuid)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResolvedEntityRef {
    /// Entity type (e.g., "fund", "limited_company")
    pub entity_type: String,
    /// Human-readable search key / display name
    pub display_name: String,
    /// Resolved UUID from EntityGateway
    pub entity_id: Uuid,
    /// Optional metadata (jurisdiction, date, etc.)
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

/// Key set for a template parameter
/// Captures what the agent has collected for a specific param
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TemplateParamKeySet {
    /// Parameter name from template (e.g., "fund_entity", "manco_entity")
    pub param_name: String,
    /// Entity type expected (e.g., "fund", "limited_company")
    pub entity_type: String,
    /// Cardinality: "batch" (one per iteration) or "shared" (same for all)
    pub cardinality: String,
    /// The collected entities
    pub entities: Vec<ResolvedEntityRef>,
    /// Whether collection is complete (user confirmed)
    #[serde(default)]
    pub is_complete: bool,
    /// Search/filter description used to find these (for audit)
    #[serde(default)]
    pub filter_description: String,
}

/// Agent's working memory for template-driven batch execution
/// This is what the agent reads/writes across conversation turns
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct TemplateExecutionContext {
    /// Template being used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    /// Current phase of template execution
    #[serde(default)]
    pub phase: TemplatePhase,

    /// Key sets collected for each template parameter
    /// Key = param_name, Value = collected entities
    #[serde(default)]
    pub key_sets: HashMap<String, TemplateParamKeySet>,

    /// Scalar params (non-entity values like jurisdiction, dates)
    #[serde(default)]
    pub scalar_params: HashMap<String, String>,

    /// Which batch item is currently being processed (0-indexed)
    #[serde(default)]
    pub current_batch_index: usize,

    /// Results from each batch item execution
    #[serde(default)]
    pub batch_results: Vec<BatchItemResult>,

    /// Whether to auto-continue without prompting
    #[serde(default)]
    pub auto_execute: bool,
}

/// Phase of template execution workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TemplatePhase {
    /// Agent is identifying which template to use
    #[default]
    SelectingTemplate,
    /// Agent is collecting shared params (same for all batch items)
    CollectingSharedParams,
    /// Agent is collecting batch params (the iteration set)
    CollectingBatchParams,
    /// User is reviewing collected key sets before execution
    ReviewingKeySets,
    /// Executing batch items one by one
    Executing,
    /// All items processed
    Complete,
}

/// Result from executing one batch item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BatchItemResult {
    /// Index in the batch
    pub index: usize,
    /// Source entity that was processed
    pub source_entity: ResolvedEntityRef,
    /// Whether execution succeeded
    pub success: bool,
    /// Created entity ID (e.g., the new CBU)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_id: Option<Uuid>,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The DSL that was executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_dsl: Option<String>,
}

impl TemplateExecutionContext {
    /// Get the batch key set (the one we iterate over)
    pub(crate) fn batch_key_set(&self) -> Option<&TemplateParamKeySet> {
        self.key_sets.values().find(|ks| ks.cardinality == "batch")
    }

    /// Get count of batch items to process
    pub(crate) fn batch_size(&self) -> usize {
        self.batch_key_set()
            .map(|ks| ks.entities.len())
            .unwrap_or(0)
    }

    /// Get current batch item being processed
    pub(crate) fn current_batch_entity(&self) -> Option<&ResolvedEntityRef> {
        self.batch_key_set()
            .and_then(|ks| ks.entities.get(self.current_batch_index))
    }

    /// Get shared entities (same for all batch items)
    pub(crate) fn shared_entities(&self) -> Vec<(&str, &ResolvedEntityRef)> {
        self.key_sets
            .iter()
            .filter(|(_, ks)| ks.cardinality == "shared")
            .flat_map(|(name, ks)| ks.entities.first().map(|e| (name.as_str(), e)))
            .collect()
    }

    /// Advance to next batch item, returns true if more to process
    pub(crate) fn advance(&mut self) -> bool {
        self.current_batch_index += 1;
        self.current_batch_index < self.batch_size()
    }

    /// Get progress string like "3/10 complete"
    pub(crate) fn progress_string(&self) -> String {
        let total = self.batch_size();
        let completed = self.batch_results.iter().filter(|r| r.success).count();
        let failed = self.batch_results.iter().filter(|r| !r.success).count();
        if failed > 0 {
            format!("{}/{} complete, {} failed", completed, total, failed)
        } else {
            format!("{}/{} complete", completed, total)
        }
    }

    /// Check if template execution is active
    pub(crate) fn is_active(&self) -> bool {
        self.template_id.is_some() && self.phase != TemplatePhase::Complete
    }

    /// Reset for a new template execution
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Batch context for bulk REPL operations
/// Holds the "working set" of entities the agent and user are processing
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct BatchContext {
    /// Whether batch mode is active
    #[serde(default)]
    pub is_active: bool,
    /// Source entity type we're iterating over (e.g., "fund")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_entity_type: Option<String>,
    /// Filter criteria used to build the set (for display/audit)
    #[serde(default)]
    pub filter_description: String,
    /// The batch working set - entities to process
    #[serde(default)]
    pub items: Vec<BatchItem>,
    /// Current index in the batch (which item is active)
    #[serde(default)]
    pub current_index: usize,
    /// Template being used for expansion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    /// Auto-continue mode (run all without prompting)
    #[serde(default)]
    pub auto_continue: bool,
    /// Common bindings shared across all batch items (e.g., @manco, @im)
    #[serde(default)]
    pub shared_bindings: HashMap<String, BoundEntity>,
}

// ============================================================================
// ResolvedEntity — Pre-resolved entity from entity-first parsing
// ============================================================================

/// An entity that was resolved from entity linking during utterance parsing.
///
/// Stored in `SessionContext.resolved_entities` keyed by the lowercased
/// mention text so that subsequent utterances referencing the same name
/// can reuse the UUID without re-resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResolvedEntity {
    /// The resolved entity UUID.
    pub entity_id: Uuid,
    /// Canonical display name from the entity snapshot.
    pub canonical_name: String,
    /// Entity kind (e.g., "cbu", "group", "company", "fund").
    pub entity_kind: String,
    /// Confidence score from entity linking (0.0–1.0).
    pub confidence: f64,
    /// Constellation slot this entity occupies, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constellation_slot: Option<String>,
}

/// Context maintained across the session for reference resolution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct SessionContext {
    /// Version of business_reference when loaded (for optimistic locking)
    /// When saving, this version must match the DB version or we get a conflict
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_dsl_version: Option<i32>,

    /// Business reference for this session's DSL instance (e.g., CBU name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_reference: Option<String>,

    /// Taxonomy navigation stack for fractal drill-down
    #[serde(default)]
    pub taxonomy_stack: ob_poc_taxonomy::taxonomy::TaxonomyStack,

    // =========================================================================
    // Taxonomy-Driven Layout Fields
    // =========================================================================
    /// Current navigation scope path (e.g., /Universe/Book/CBU/Entity)
    /// Tracks where the user is in the hierarchical navigation
    #[serde(default)]
    pub scope_path: crate::session::ScopePath,

    /// Structural mass of the current scope - weighted complexity measure
    /// Used for automatic view mode selection (Detail < SolarSystem < Universe)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub struct_mass: Option<crate::session::StructMass>,

    /// Cached mass breakdown for quick access without recomputing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass_breakdown: Option<crate::session::MassBreakdown>,

    /// Auto-selected view mode based on structural mass
    /// Overridden if user manually selects a view mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_view_mode: Option<crate::session::MassViewMode>,

    /// Whether the current view_mode was manually selected (overrides auto)
    #[serde(default)]
    pub view_mode_manual: bool,

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
    /// Dominant entity from entity linking (highest confidence mention in last utterance)
    /// Used for implicit entity reference resolution in DSL generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dominant_entity_id: Option<Uuid>,
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
    /// Currently focused stage in the onboarding journey
    /// When set, agent verb suggestions are filtered to this stage's relevant verbs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_focus: Option<String>,
    /// Primary domain keys - the main identifiers for this onboarding session
    #[serde(default)]
    pub primary_keys: PrimaryDomainKeys,
    /// Batch context for bulk REPL operations (legacy - use template_execution instead)
    /// When active, session iterates over a set of entities (e.g., funds → CBUs)
    #[serde(default)]
    pub batch: BatchContext,
    /// Current session mode - determines how input is processed
    #[serde(default)]
    pub mode: SessionMode,
    /// Template execution context - agent's working memory for batch operations
    /// This holds the key sets, template state, and execution progress
    #[serde(default)]
    pub template_execution: TemplateExecutionContext,

    /// Research macro state - tracks pending results and approvals
    #[serde(default)]
    pub research: crate::session::ResearchContext,

    /// View state from view.* operations (universe, book, cbu, entity-forest)
    /// This captures what the user is currently viewing - the unified "it" that
    /// operations target. Populated after DSL execution when view.* verbs run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_state: Option<crate::session::ViewState>,

    /// Viewport state from viewport.* DSL verbs (focus, enhance, filter, camera)
    /// This tracks the CBU-focused viewport with focus state machine, enhance levels,
    /// camera state, and filters. Populated after DSL execution when viewport.* verbs run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_state: Option<ob_poc_types::ViewportState>,

    /// Session scope from session.* DSL verbs (set-galaxy, set-cbu, set-jurisdiction, etc.)
    /// This defines what data the user is operating on - the "where" for all operations.
    /// Populated after DSL execution when session.* scope verbs run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<crate::session::SessionScope>,

    // =========================================================================
    // Client Scope Context - For entity resolution within client group
    // =========================================================================
    /// Client group scope from Stage 0 scope resolution.
    /// When set, entity searches are filtered to this client group.
    /// Set by IntentPipeline when user says "work on allianz" etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_scope: Option<ScopeContext>,

    // =========================================================================
    // Deal Context - For deal taxonomy navigation
    // =========================================================================
    /// Current deal ID (for deal taxonomy navigation)
    /// Set by session.load-deal verb. When set, UI shows deal taxonomy panel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_id: Option<Uuid>,

    /// Deal name for display in UI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_name: Option<String>,

    /// Whether the deal selection gate was skipped (don't re-prompt)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deal_gate_skipped: bool,

    // =========================================================================
    // View State Fields - For REPL/View synchronization
    // =========================================================================
    /// Current view mode (e.g., "KYC_UBO", "UBO_ONLY", "SERVICE_DELIVERY", "PRODUCTS_ONLY")
    /// Determines which layers and edges are visible in the graph visualization.
    /// Syncs between UI graph panel and REPL session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_mode: Option<String>,

    /// Current zoom level for the graph visualization (0.0 - 1.0+ range)
    /// Persisted so zoom state is maintained across page refreshes and
    /// synchronized between REPL and graph view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom_level: Option<f32>,

    /// Set of expanded node IDs in the current view.
    /// Tracks which nodes have been expanded to show children.
    /// Used for fractal navigation persistence.
    #[serde(default, skip_serializing_if = "std::collections::HashSet::is_empty")]
    pub expanded_nodes: std::collections::HashSet<Uuid>,

    // =========================================================================
    // DSL Diff Tracking (for learning from user edits)
    // =========================================================================
    /// DSL as proposed by agent (before user edits in REPL)
    /// Set when agent generates DSL, compared against final_dsl on execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_dsl: Option<String>,

    /// Current DSL in REPL (may differ from proposed if user edited)
    /// Updated via REPL edit events, used to compute diff on execute
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_dsl: Option<String>,

    /// Selected discovery domain from the Sage bootstrap navigator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_selected_domain: Option<String>,

    /// Selected constellation family from the Sage bootstrap navigator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_selected_family: Option<String>,

    /// Selected concrete constellation from the Sage bootstrap navigator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_selected_constellation: Option<String>,

    /// Structured answers collected during Sage bootstrap.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub discovery_answers: HashMap<String, String>,

    // =========================================================================
    // Pre-Resolved Entities — Entity-First Parsing (PR 1)
    // =========================================================================
    /// Entities resolved from entity linking during the current session.
    /// Keyed by the mention text (lowercased) that was resolved, so subsequent
    /// utterances referencing the same name can reuse the UUID without
    /// re-resolution.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resolved_entities: HashMap<String, ResolvedEntity>,
}

/// Primary domain keys tracked across the session
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PrimaryDomainKeys {
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
    /// Set a typed binding with display name
    /// Returns the actual binding name used (may have suffix if collision)
    ///
    /// Special handling for "cbu" binding: always replaces (no suffix) since
    /// the UI's active CBU should always be @cbu, not @cbu_2, @cbu_3, etc.
    pub(crate) fn set_binding(
        &mut self,
        name: &str,
        id: Uuid,
        entity_type: &str,
        display_name: &str,
    ) -> String {
        // Special case: "cbu" binding always replaces - the UI's active CBU
        // should always be accessible as @cbu, not @cbu_2, @cbu_3
        let actual_name = if name == "cbu" {
            name.to_string()
        } else if self.bindings.contains_key(name) {
            // Handle collision - append suffix if name already exists
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

    /// Set the active CBU for this session
    pub(crate) fn set_active_cbu(&mut self, id: Uuid, display_name: &str) {
        self.active_cbu = Some(BoundEntity {
            id,
            entity_type: "cbu".to_string(),
            display_name: display_name.to_string(),
        });
    }

    // =========================================================================
    // AST MANIPULATION
    // =========================================================================

    /// Add statements to the AST
    pub(crate) fn add_statements(&mut self, statements: Vec<Statement>) {
        for stmt in statements {
            self.add_statement(stmt);
        }
    }

    /// Add a single statement to the AST, indexing by binding name if present
    pub(crate) fn add_statement(&mut self, statement: Statement) {
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

    /// Update primary keys from execution result
    pub(crate) fn update_primary_key(&mut self, domain: &str, binding: &str, id: Uuid) {
        match domain {
            "cbu" if self.primary_keys.cbu_id.is_none() => {
                self.primary_keys.cbu_id = Some(id);
            }
            "kyc-case" if self.primary_keys.kyc_case_id.is_none() => {
                self.primary_keys.kyc_case_id = Some(id);
            }
            "service-resource" if self.primary_keys.resource_instance_id.is_none() => {
                self.primary_keys.resource_instance_id = Some(id);
            }
            _ => {}
        }
        // Also index by the specific binding name
        if let Some(idx) = self.ast_index.get(binding) {
            // Already indexed when statement was added
            let _ = idx;
        }
    }

    /// Render the AST back to DSL source for display
    pub(crate) fn to_dsl_source(&self) -> String {
        self.ast
            .iter()
            .map(|s| s.to_dsl_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get count of statements
    pub(crate) fn statement_count(&self) -> usize {
        self.ast.len()
    }

    // =========================================================================
    // VIEW STATE METHODS - For view.* verb output propagation
    // =========================================================================

    /// Set the view state from view.* operations
    /// This is called after DSL execution when view.* verbs produce a ViewState
    pub(crate) fn set_view_state(&mut self, view: crate::session::ViewState) {
        self.view_state = Some(view);
    }

    // =========================================================================
    // VIEWPORT STATE METHODS - For viewport.* verb output propagation
    // =========================================================================

    /// Set the viewport state from viewport.* operations
    /// This is called after DSL execution when viewport.* verbs produce a ViewportState
    pub(crate) fn set_viewport_state(&mut self, state: ob_poc_types::ViewportState) {
        self.viewport_state = Some(state);
    }

    // =========================================================================
    // SCOPE METHODS - For session.* verb output propagation
    // =========================================================================

    /// Set the session scope from session.* operations
    /// This is called after DSL execution when session.set-* verbs produce a scope change
    pub(crate) fn set_scope(&mut self, scope: crate::session::SessionScope) {
        self.scope = Some(scope);
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
    /// Result data for Record/RecordSet operations (e.g., cbu.inspect)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

// ============================================================================
// Session Store
// ============================================================================

/// Thread-safe in-memory session store
///
/// NOTE: This now uses `UnifiedSession` as the single session type.
/// `AgentSession` is deprecated and will be removed in a future version.
pub(crate) type SessionStore = Arc<RwLock<HashMap<Uuid, crate::session::UnifiedSession>>>;

/// Create a new session store
pub fn create_session_store() -> SessionStore {
    Arc::new(RwLock::new(HashMap::new()))
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/// Reference to a client for session scope initialization
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct InitialClientRef {
    /// Client group ID (if known)
    pub client_id: Option<Uuid>,
    /// Client name/alias to search for (if client_id not known)
    pub client_name: Option<String>,
}

/// Request to create a new session
#[derive(Debug, Deserialize)]
pub(crate) struct CreateSessionRequest {
    /// Optional domain hint to focus generation
    pub domain_hint: Option<String>,
    /// Optional initial client to set session scope
    /// If provided, the session starts in Scoped state with the client context set
    /// If not provided, session starts in New state and prompts for client selection
    #[serde(default)]
    pub initial_client: Option<InitialClientRef>,
    /// Optional structure type to filter by (pe, sicav, hedge, etc.)
    #[serde(default)]
    pub structure_type: Option<String>,
    /// Optional workflow focus (e.g., "semantic-os").
    /// When set to "semantic-os", the session starts with a workflow selection
    /// decision packet instead of the normal client group resolution.
    #[serde(default)]
    pub workflow_focus: Option<String>,
}

/// Welcome message constant - agent's opening question
pub(crate) const WELCOME_MESSAGE: &str = "Which client would you like to work with today?";

/// Response after creating a session
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    /// The new session ID
    pub session_id: Uuid,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// Initial state (always AwaitingScope until client/CBU set selected)
    pub state: SessionState,
    /// Welcome message from agent - asks for scope selection
    pub welcome_message: String,
    /// Decision packet for client group selection (populated on new sessions without initial_client)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<ob_poc_types::DecisionPacket>,
    /// Session feedback (universe root state — workspace options + bootstrap verbs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_feedback: Option<serde_json::Value>,
}

// ChatRequest is now in ob-poc-types - SINGLE source of truth

/// Response with session state
#[derive(Debug, Serialize)]
pub(crate) struct SessionStateResponse {
    /// Session ID
    pub session_id: Uuid,
    /// Entity type this session operates on ("cbu", "kyc_case", "onboarding", "bulk", etc.)
    pub entity_type: String,
    /// Entity ID this session operates on (None if creating new or bulk mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<Uuid>,
    /// Current state
    pub state: SessionState,
    /// Number of messages in the session
    pub message_count: usize,
    /// Combined DSL (None if no DSL assembled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combined_dsl: Option<String>,
    /// Session context
    pub context: SessionContext,
    /// Conversation history (empty vec if none)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<ChatMessage>,
    /// Whether the session can execute
    pub can_execute: bool,
    /// Session version (ISO timestamp from updated_at)
    /// UI uses this to detect external changes (MCP/REPL modifying session)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Run sheet - DSL statement ledger with per-statement status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_sheet: Option<ob_poc_types::RunSheet>,
    /// Symbol bindings in this session
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub bindings: std::collections::HashMap<String, ob_poc_types::BoundEntityInfo>,
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
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_store() {
        use crate::session::UnifiedSession;

        let store = create_session_store();
        let session = UnifiedSession::new();
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
            // UnifiedSession uses crate::session::unified::SessionState
            assert_eq!(
                read.get(&id).unwrap().state,
                crate::session::unified::SessionState::New
            );
        }
    }
}
