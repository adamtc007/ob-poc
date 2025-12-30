//! Enhanced Agent Context Builder
//!
//! Combines session context (bindings, bootstrap) with verb discovery
//! to provide rich, context-aware prompts for the LLM agent.
//!
//! This module lives in the main crate because it requires database access
//! for verb discovery. It wraps the base AgentContext from ob-agentic.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use ob_agentic::context_builder::{AgentContext, AgentContextBuilder, BindingDisplay};

use super::verb_discovery::{
    AgentVerbContext, DiscoveryQuery, VerbDiscoveryError, VerbDiscoveryService, VerbSuggestion,
};
use super::UnifiedSessionContext;

/// Enhanced context combining session state and verb discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedAgentContext {
    /// Base context (bindings, bootstrap hints)
    #[serde(flatten)]
    pub base: SerializableAgentContext,

    /// Verb suggestions from discovery service
    pub verb_context: Option<AgentVerbContext>,

    /// Formatted prompt section (ready for injection)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_section: Option<String>,
}

/// Serializable version of AgentContext
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableAgentContext {
    pub bindings: Vec<SerializableBinding>,
    pub needs_bootstrap: bool,
    pub cbu_id: Option<String>,
    pub suggestions: Vec<String>,
}

/// Serializable binding display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableBinding {
    pub name: String,
    pub binding_type: String,
    pub subtype: Option<String>,
    pub display_name: Option<String>,
    pub uuid: Option<String>,
}

impl From<&BindingDisplay> for SerializableBinding {
    fn from(b: &BindingDisplay) -> Self {
        Self {
            name: b.name.clone(),
            binding_type: b.binding_type.clone(),
            subtype: b.subtype.clone(),
            display_name: b.display_name.clone(),
            uuid: b.uuid.map(|u| u.to_string()),
        }
    }
}

impl From<&AgentContext> for SerializableAgentContext {
    fn from(ctx: &AgentContext) -> Self {
        Self {
            bindings: ctx.bindings.iter().map(SerializableBinding::from).collect(),
            needs_bootstrap: ctx.needs_bootstrap,
            cbu_id: ctx.cbu_id.map(|u| u.to_string()),
            suggestions: ctx.suggestions.clone(),
        }
    }
}

/// Builder for enhanced agent context
pub struct EnhancedContextBuilder {
    discovery_service: VerbDiscoveryService,
    bindings: HashMap<String, Uuid>,
    cbu_id: Option<Uuid>,
    graph_context: Option<String>,
    workflow_phase: Option<String>,
    recent_verbs: Vec<String>,
    user_intent: Option<String>,
}

impl EnhancedContextBuilder {
    /// Create a new builder with database connection
    pub fn new(pool: Arc<PgPool>) -> Self {
        let discovery_service = VerbDiscoveryService::new(pool);
        Self {
            discovery_service,
            bindings: HashMap::new(),
            cbu_id: None,
            graph_context: None,
            workflow_phase: None,
            recent_verbs: Vec::new(),
            user_intent: None,
        }
    }

    /// Add session bindings
    pub fn with_bindings(mut self, bindings: HashMap<String, Uuid>) -> Self {
        self.bindings = bindings;
        self
    }

    /// Set the active CBU
    pub fn with_cbu(mut self, cbu_id: Option<Uuid>) -> Self {
        self.cbu_id = cbu_id;
        self
    }

    /// Set graph context (e.g., "cursor_on_entity", "layer_ubo")
    pub fn with_graph_context(mut self, context: Option<String>) -> Self {
        self.graph_context = context;
        self
    }

    /// Set workflow phase (e.g., "entity_collection", "screening")
    pub fn with_workflow_phase(mut self, phase: Option<String>) -> Self {
        self.workflow_phase = phase;
        self
    }

    /// Set recently used verbs
    pub fn with_recent_verbs(mut self, verbs: Vec<String>) -> Self {
        self.recent_verbs = verbs;
        self
    }

    /// Set user intent text (for verb discovery)
    pub fn with_user_intent(mut self, intent: Option<String>) -> Self {
        self.user_intent = intent;
        self
    }

    /// Build from a UnifiedSessionContext
    pub fn from_session_context(pool: Arc<PgPool>, session: &UnifiedSessionContext) -> Self {
        let mut builder = Self::new(pool);

        // Extract bindings from execution context
        for (name, uuid) in &session.execution.symbols {
            builder.bindings.insert(name.clone(), *uuid);
        }

        // Find CBU binding if present
        for (name, uuid) in &session.execution.symbols {
            if let Some(binding_type) = session.execution.symbol_types.get(name) {
                if binding_type == "cbu" {
                    builder.cbu_id = Some(*uuid);
                    break;
                }
            }
        }

        // Extract graph context from cursor position
        if let Some(ref graph) = session.graph {
            if let Some(cursor_id) = graph.cursor {
                if let Some(node) = graph.nodes.get(&cursor_id) {
                    let entity_type = format!("{:?}", node.entity_type).to_lowercase();
                    builder.graph_context = Some(format!("cursor_on_{}", entity_type));
                }
            }
        }

        // Note: workflow_phase and recent_verbs need to be set separately
        // as they're not stored in UnifiedSessionContext

        builder
    }

    /// Build the enhanced context (async because verb discovery needs DB)
    pub async fn build(self) -> Result<EnhancedAgentContext, VerbDiscoveryError> {
        // Build base context using ob-agentic builder
        let base_context = AgentContextBuilder::new()
            .with_bindings_map(&self.bindings)
            .with_cbu_id(self.cbu_id)
            .build();

        // Get verb suggestions if we have any context
        let verb_context = if self.user_intent.is_some()
            || self.graph_context.is_some()
            || self.workflow_phase.is_some()
            || !self.recent_verbs.is_empty()
        {
            Some(
                self.discovery_service
                    .build_suggestions_for_agent(
                        self.user_intent.as_deref(),
                        self.graph_context.as_deref(),
                        self.workflow_phase.as_deref(),
                        &self.recent_verbs,
                    )
                    .await?,
            )
        } else {
            None
        };

        // Build the combined prompt section
        let prompt_section = build_prompt_section(&base_context, verb_context.as_ref());

        Ok(EnhancedAgentContext {
            base: SerializableAgentContext::from(&base_context),
            verb_context,
            prompt_section: Some(prompt_section),
        })
    }
}

/// Build the complete prompt section from context
fn build_prompt_section(base: &AgentContext, verbs: Option<&AgentVerbContext>) -> String {
    let mut parts = Vec::new();

    // 1. Bootstrap hint if needed
    if let Some(hint) = base.format_bootstrap_hint() {
        parts.push(hint);
    }

    // 2. Available bindings
    let bindings_str = base.format_bindings_for_llm();
    if !bindings_str.is_empty() {
        parts.push(bindings_str);
    }

    // 3. Verb suggestions
    if let Some(verb_ctx) = verbs {
        if !verb_ctx.is_empty() {
            parts.push(verb_ctx.to_prompt_text());
        }
    }

    // 4. Base suggestions (fallback if no verb context)
    if verbs.map_or(true, |v| v.is_empty()) && !base.suggestions.is_empty() {
        let suggestions_str = format!("[SUGGESTIONS: {}]", base.suggestions.join(" | "));
        parts.push(suggestions_str);
    }

    parts.join("\n\n")
}

/// Quick helper to get verb suggestions for a user query
pub async fn get_verb_suggestions(
    pool: &PgPool,
    query: &str,
    graph_context: Option<&str>,
    workflow_phase: Option<&str>,
    limit: usize,
) -> Result<Vec<VerbSuggestion>, VerbDiscoveryError> {
    let service = VerbDiscoveryService::new(Arc::new(pool.clone()));

    let mut discovery_query = DiscoveryQuery::new().with_query(query).with_limit(limit);

    if let Some(ctx) = graph_context {
        discovery_query = discovery_query.with_graph_context(ctx);
    }
    if let Some(phase) = workflow_phase {
        discovery_query = discovery_query.with_workflow_phase(phase);
    }

    service.discover(&discovery_query).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serializable_binding_from() {
        let binding = BindingDisplay {
            name: "fund".to_string(),
            binding_type: "cbu".to_string(),
            subtype: None,
            display_name: Some("Apex Fund".to_string()),
            uuid: Some(Uuid::new_v4()),
        };

        let serializable = SerializableBinding::from(&binding);
        assert_eq!(serializable.name, "fund");
        assert_eq!(serializable.binding_type, "cbu");
        assert!(serializable.uuid.is_some());
    }

    #[test]
    fn test_prompt_section_with_bootstrap() {
        let base = AgentContextBuilder::new().build();
        let prompt = build_prompt_section(&base, None);

        assert!(prompt.contains("NEW CBU MODE"));
        assert!(prompt.contains("cbu.ensure"));
    }

    #[test]
    fn test_prompt_section_with_bindings() {
        let mut bindings = HashMap::new();
        bindings.insert("fund".to_string(), Uuid::new_v4());

        let base = AgentContextBuilder::new()
            .with_bindings_map(&bindings)
            .build();
        let prompt = build_prompt_section(&base, None);

        assert!(prompt.contains("@fund"));
        assert!(!prompt.contains("NEW CBU MODE")); // No bootstrap needed
    }
}
