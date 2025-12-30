//! Verb Discovery Service
//!
//! Provides intelligent verb suggestions based on:
//! - User intent (natural language patterns via full-text search)
//! - Graph context (cursor position, selected node, active layer)
//! - Workflow phase (KYC lifecycle state)
//!
//! Queries the `dsl_verbs` table which is synced from YAML on startup.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use thiserror::Error;

/// A verb suggestion with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSuggestion {
    /// Full verb name (e.g., "cbu.assign-role")
    pub verb: String,
    /// Domain (e.g., "cbu")
    pub domain: String,
    /// Short description of what the verb does
    pub description: Option<String>,
    /// Example DSL usage
    pub example: Option<String>,
    /// Category (e.g., "entity_management")
    pub category: Option<String>,
    /// Relevance score (higher = more relevant)
    pub score: f32,
    /// Why this verb was suggested
    pub reason: SuggestionReason,
}

/// Reason why a verb was suggested
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SuggestionReason {
    /// Matched user intent via full-text search
    IntentMatch { query: String, rank: f32 },
    /// Matched intent pattern
    PatternMatch { pattern: String },
    /// Relevant to current graph context
    GraphContext { context_type: String },
    /// Applicable to current workflow phase
    WorkflowPhase { phase: String },
    /// In same category as recent verbs
    CategoryMatch { category: String },
    /// Suggested as typical next verb
    TypicalNext { after_verb: String },
    /// General suggestion
    General,
}

/// Discovery query parameters
#[derive(Debug, Clone, Default)]
pub struct DiscoveryQuery {
    /// Natural language query text (for full-text search)
    pub query_text: Option<String>,
    /// Current graph context (e.g., "cursor_on_cbu", "layer_ubo")
    pub graph_context: Option<String>,
    /// Current workflow phase (e.g., "entity_collection", "screening")
    pub workflow_phase: Option<String>,
    /// Recently used verbs (for category matching and typical_next)
    pub recent_verbs: Vec<String>,
    /// Filter by category
    pub category: Option<String>,
    /// Filter by domain
    pub domain: Option<String>,
    /// Maximum number of suggestions to return
    pub limit: usize,
}

impl DiscoveryQuery {
    pub fn new() -> Self {
        Self {
            limit: 10,
            ..Default::default()
        }
    }

    pub fn with_query(mut self, text: &str) -> Self {
        self.query_text = Some(text.to_string());
        self
    }

    pub fn with_graph_context(mut self, context: &str) -> Self {
        self.graph_context = Some(context.to_string());
        self
    }

    pub fn with_workflow_phase(mut self, phase: &str) -> Self {
        self.workflow_phase = Some(phase.to_string());
        self
    }

    pub fn with_recent_verbs(mut self, verbs: Vec<String>) -> Self {
        self.recent_verbs = verbs;
        self
    }

    pub fn with_category(mut self, category: &str) -> Self {
        self.category = Some(category.to_string());
        self
    }

    pub fn with_domain(mut self, domain: &str) -> Self {
        self.domain = Some(domain.to_string());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// DB row for verb discovery
/// Fields are required for SQLx query binding even if not directly used in Rust code
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
struct VerbRow {
    pub full_name: String,
    pub domain: String,
    pub verb_name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub example_short: Option<String>,
    pub example_dsl: Option<String>,
    pub intent_patterns: Option<Vec<String>>,
    pub workflow_phases: Option<Vec<String>>,
    pub graph_contexts: Option<Vec<String>>,
    pub typical_next: Option<Vec<String>>,
}

/// DB row for full-text search results (includes rank)
/// Fields are required for SQLx query binding even if not directly used in Rust code
#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
struct VerbRowWithRank {
    pub full_name: String,
    pub domain: String,
    pub verb_name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub example_short: Option<String>,
    pub example_dsl: Option<String>,
    pub intent_patterns: Option<Vec<String>>,
    pub workflow_phases: Option<Vec<String>>,
    pub graph_contexts: Option<Vec<String>>,
    pub typical_next: Option<Vec<String>>,
    pub rank: f32,
}

/// Service for discovering relevant verbs from database
pub struct VerbDiscoveryService {
    pool: Arc<PgPool>,
}

impl VerbDiscoveryService {
    /// Create a new VerbDiscoveryService with database connection
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    /// Discover relevant verbs based on query
    pub async fn discover(
        &self,
        query: &DiscoveryQuery,
    ) -> Result<Vec<VerbSuggestion>, VerbDiscoveryError> {
        let mut suggestions: Vec<VerbSuggestion> = Vec::new();
        let mut seen_verbs: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 1. Full-text search on search_text if query provided
        if let Some(ref text) = query.query_text {
            let fts_results = self.search_by_text(text, query.limit).await?;
            for row in fts_results {
                if seen_verbs.insert(row.full_name.clone()) {
                    suggestions.push(VerbSuggestion {
                        verb: row.full_name.clone(),
                        domain: row.domain.clone(),
                        description: row.description.clone(),
                        example: row.example_dsl.clone(),
                        category: row.category.clone(),
                        score: row.rank,
                        reason: SuggestionReason::IntentMatch {
                            query: text.clone(),
                            rank: row.rank,
                        },
                    });
                }
            }

            // Also check intent_patterns for exact matches
            let pattern_results = self.search_by_patterns(text, query.limit).await?;
            for (verb, pattern) in pattern_results {
                if seen_verbs.insert(verb.full_name.clone()) {
                    suggestions.push(VerbSuggestion {
                        verb: verb.full_name.clone(),
                        domain: verb.domain.clone(),
                        description: verb.description.clone(),
                        example: verb.example_dsl.clone(),
                        category: verb.category.clone(),
                        score: 0.95, // High score for pattern match
                        reason: SuggestionReason::PatternMatch { pattern },
                    });
                }
            }
        }

        // 2. Match against graph context
        if let Some(ref context) = query.graph_context {
            let context_results = self.search_by_graph_context(context, query.limit).await?;
            for verb in context_results {
                if seen_verbs.insert(verb.full_name.clone()) {
                    suggestions.push(VerbSuggestion {
                        verb: verb.full_name.clone(),
                        domain: verb.domain.clone(),
                        description: verb.description.clone(),
                        example: verb.example_dsl.clone(),
                        category: verb.category.clone(),
                        score: 0.85,
                        reason: SuggestionReason::GraphContext {
                            context_type: context.clone(),
                        },
                    });
                }
            }
        }

        // 3. Match against workflow phase
        if let Some(ref phase) = query.workflow_phase {
            let phase_results = self.search_by_workflow_phase(phase, query.limit).await?;
            for verb in phase_results {
                if seen_verbs.insert(verb.full_name.clone()) {
                    suggestions.push(VerbSuggestion {
                        verb: verb.full_name.clone(),
                        domain: verb.domain.clone(),
                        description: verb.description.clone(),
                        example: verb.example_dsl.clone(),
                        category: verb.category.clone(),
                        score: 0.80,
                        reason: SuggestionReason::WorkflowPhase {
                            phase: phase.clone(),
                        },
                    });
                }
            }
        }

        // 4. Get typical_next suggestions based on recent verbs
        if !query.recent_verbs.is_empty() {
            if let Some(last_verb) = query.recent_verbs.last() {
                let next_results = self.get_typical_next(last_verb, query.limit).await?;
                for verb in next_results {
                    if seen_verbs.insert(verb.full_name.clone()) {
                        suggestions.push(VerbSuggestion {
                            verb: verb.full_name.clone(),
                            domain: verb.domain.clone(),
                            description: verb.description.clone(),
                            example: verb.example_dsl.clone(),
                            category: verb.category.clone(),
                            score: 0.70,
                            reason: SuggestionReason::TypicalNext {
                                after_verb: last_verb.clone(),
                            },
                        });
                    }
                }
            }

            // Also suggest verbs in same category as recent verbs
            let categories = self.get_categories_for_verbs(&query.recent_verbs).await?;
            for category in categories {
                let cat_results = self.search_by_category(&category, query.limit).await?;
                for verb in cat_results {
                    if seen_verbs.insert(verb.full_name.clone()) {
                        suggestions.push(VerbSuggestion {
                            verb: verb.full_name.clone(),
                            domain: verb.domain.clone(),
                            description: verb.description.clone(),
                            example: verb.example_dsl.clone(),
                            category: verb.category.clone(),
                            score: 0.50,
                            reason: SuggestionReason::CategoryMatch {
                                category: category.clone(),
                            },
                        });
                    }
                }
            }
        }

        // Sort by score descending
        suggestions.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply limit
        suggestions.truncate(query.limit);

        Ok(suggestions)
    }

    /// Full-text search on search_text column
    async fn search_by_text(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VerbRowWithRank>, VerbDiscoveryError> {
        let rows: Vec<VerbRowWithRank> = sqlx::query_as(
            r#"
            SELECT
                full_name, domain, verb_name, description, category,
                example_short, example_dsl, intent_patterns,
                workflow_phases, graph_contexts, typical_next,
                ts_rank(to_tsvector('english', coalesce(search_text, '')),
                        plainto_tsquery('english', $1))::real as rank
            FROM "ob-poc".dsl_verbs
            WHERE to_tsvector('english', coalesce(search_text, '')) @@ plainto_tsquery('english', $1)
            ORDER BY rank DESC
            LIMIT $2
            "#,
        )
        .bind(query)
        .bind(limit as i64)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(rows)
    }

    /// Search by intent patterns (array contains)
    async fn search_by_patterns(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(VerbRow, String)>, VerbDiscoveryError> {
        let query_lower = query.to_lowercase();

        // Get all verbs with intent_patterns and filter in Rust
        // (more flexible than SQL array matching for partial patterns)
        let rows: Vec<VerbRow> = sqlx::query_as(
            r#"
            SELECT
                full_name, domain, verb_name, description, category,
                example_short, example_dsl, intent_patterns,
                workflow_phases, graph_contexts, typical_next
            FROM "ob-poc".dsl_verbs
            WHERE intent_patterns IS NOT NULL AND array_length(intent_patterns, 1) > 0
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        let mut results = Vec::new();
        for row in rows {
            if let Some(ref patterns) = row.intent_patterns {
                for pattern in patterns {
                    if query_lower.contains(&pattern.to_lowercase())
                        || pattern.to_lowercase().contains(&query_lower)
                    {
                        results.push((row.clone(), pattern.clone()));
                        break;
                    }
                }
            }
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    /// Search by graph context
    async fn search_by_graph_context(
        &self,
        context: &str,
        limit: usize,
    ) -> Result<Vec<VerbRow>, VerbDiscoveryError> {
        let rows: Vec<VerbRow> = sqlx::query_as(
            r#"
            SELECT
                full_name, domain, verb_name, description, category,
                example_short, example_dsl, intent_patterns,
                workflow_phases, graph_contexts, typical_next
            FROM "ob-poc".dsl_verbs
            WHERE $1 = ANY(graph_contexts)
            LIMIT $2
            "#,
        )
        .bind(context)
        .bind(limit as i64)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(rows)
    }

    /// Search by workflow phase
    async fn search_by_workflow_phase(
        &self,
        phase: &str,
        limit: usize,
    ) -> Result<Vec<VerbRow>, VerbDiscoveryError> {
        let rows: Vec<VerbRow> = sqlx::query_as(
            r#"
            SELECT
                full_name, domain, verb_name, description, category,
                example_short, example_dsl, intent_patterns,
                workflow_phases, graph_contexts, typical_next
            FROM "ob-poc".dsl_verbs
            WHERE $1 = ANY(workflow_phases)
            LIMIT $2
            "#,
        )
        .bind(phase)
        .bind(limit as i64)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(rows)
    }

    /// Search by category
    async fn search_by_category(
        &self,
        category: &str,
        limit: usize,
    ) -> Result<Vec<VerbRow>, VerbDiscoveryError> {
        let rows: Vec<VerbRow> = sqlx::query_as(
            r#"
            SELECT
                full_name, domain, verb_name, description, category,
                example_short, example_dsl, intent_patterns,
                workflow_phases, graph_contexts, typical_next
            FROM "ob-poc".dsl_verbs
            WHERE category = $1
            LIMIT $2
            "#,
        )
        .bind(category)
        .bind(limit as i64)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(rows)
    }

    /// Get typical next verbs for a given verb
    async fn get_typical_next(
        &self,
        verb: &str,
        limit: usize,
    ) -> Result<Vec<VerbRow>, VerbDiscoveryError> {
        // First get the typical_next array for the given verb
        let next_verbs: Option<Vec<String>> = sqlx::query_scalar(
            r#"SELECT typical_next FROM "ob-poc".dsl_verbs WHERE full_name = $1"#,
        )
        .bind(verb)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?
        .flatten();

        if let Some(next) = next_verbs {
            if !next.is_empty() {
                let rows: Vec<VerbRow> = sqlx::query_as(
                    r#"
                    SELECT
                        full_name, domain, verb_name, description, category,
                        example_short, example_dsl, intent_patterns,
                        workflow_phases, graph_contexts, typical_next
                    FROM "ob-poc".dsl_verbs
                    WHERE full_name = ANY($1)
                    LIMIT $2
                    "#,
                )
                .bind(&next)
                .bind(limit as i64)
                .fetch_all(self.pool.as_ref())
                .await
                .map_err(VerbDiscoveryError::Database)?;

                return Ok(rows);
            }
        }

        Ok(Vec::new())
    }

    /// Get categories for a list of verbs
    async fn get_categories_for_verbs(
        &self,
        verbs: &[String],
    ) -> Result<Vec<String>, VerbDiscoveryError> {
        let categories: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT category
            FROM "ob-poc".dsl_verbs
            WHERE full_name = ANY($1) AND category IS NOT NULL
            "#,
        )
        .bind(verbs)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(categories)
    }

    /// Get verbs for a specific category
    pub async fn get_category_verbs(
        &self,
        category: &str,
    ) -> Result<Vec<String>, VerbDiscoveryError> {
        let verbs: Vec<String> = sqlx::query_scalar(
            r#"SELECT full_name FROM "ob-poc".dsl_verbs WHERE category = $1 ORDER BY full_name"#,
        )
        .bind(category)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(verbs)
    }

    /// Get verbs for a specific workflow phase
    pub async fn get_phase_verbs(&self, phase: &str) -> Result<Vec<String>, VerbDiscoveryError> {
        let verbs: Vec<String> = sqlx::query_scalar(
            r#"SELECT full_name FROM "ob-poc".dsl_verbs WHERE $1 = ANY(workflow_phases) ORDER BY full_name"#,
        )
        .bind(phase)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(verbs)
    }

    /// Get all available categories
    pub async fn list_categories(&self) -> Result<Vec<CategoryInfo>, VerbDiscoveryError> {
        let rows: Vec<CategoryInfo> = sqlx::query_as(
            r#"
            SELECT category_code, label, description, display_order
            FROM "ob-poc".dsl_verb_categories
            ORDER BY display_order
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(rows)
    }

    /// Get all workflow phases in order
    pub async fn list_workflow_phases(&self) -> Result<Vec<WorkflowPhaseInfo>, VerbDiscoveryError> {
        let rows: Vec<WorkflowPhaseInfo> = sqlx::query_as(
            r#"
            SELECT phase_code, label, description, phase_order, transitions_to
            FROM "ob-poc".dsl_workflow_phases
            ORDER BY phase_order
            "#,
        )
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?;

        Ok(rows)
    }

    /// Get example for a verb
    pub async fn get_verb_example(&self, verb: &str) -> Result<Option<String>, VerbDiscoveryError> {
        let example: Option<String> = sqlx::query_scalar(
            r#"SELECT example_dsl FROM "ob-poc".dsl_verbs WHERE full_name = $1"#,
        )
        .bind(verb)
        .fetch_optional(self.pool.as_ref())
        .await
        .map_err(VerbDiscoveryError::Database)?
        .flatten();

        Ok(example)
    }

    /// Build context-aware suggestions for the agent
    pub async fn build_suggestions_for_agent(
        &self,
        query_text: Option<&str>,
        graph_context: Option<&str>,
        workflow_phase: Option<&str>,
        recent_verbs: &[String],
    ) -> Result<AgentVerbContext, VerbDiscoveryError> {
        let mut query = DiscoveryQuery::new().with_limit(15);

        if let Some(text) = query_text {
            query = query.with_query(text);
        }
        if let Some(ctx) = graph_context {
            query = query.with_graph_context(ctx);
        }
        if let Some(phase) = workflow_phase {
            query = query.with_workflow_phase(phase);
        }
        if !recent_verbs.is_empty() {
            query = query.with_recent_verbs(recent_verbs.to_vec());
        }

        let suggestions = self.discover(&query).await?;

        // Group by reason type for structured output
        let mut by_intent: Vec<VerbSuggestion> = Vec::new();
        let mut by_context: Vec<VerbSuggestion> = Vec::new();
        let mut by_phase: Vec<VerbSuggestion> = Vec::new();
        let mut by_category: Vec<VerbSuggestion> = Vec::new();

        for suggestion in suggestions {
            match &suggestion.reason {
                SuggestionReason::IntentMatch { .. } | SuggestionReason::PatternMatch { .. } => {
                    by_intent.push(suggestion)
                }
                SuggestionReason::GraphContext { .. } => by_context.push(suggestion),
                SuggestionReason::WorkflowPhase { .. } => by_phase.push(suggestion),
                SuggestionReason::CategoryMatch { .. } | SuggestionReason::TypicalNext { .. } => {
                    by_category.push(suggestion)
                }
                SuggestionReason::General => by_category.push(suggestion),
            }
        }

        Ok(AgentVerbContext {
            by_intent,
            by_context,
            by_phase,
            by_category,
            current_phase: workflow_phase.map(String::from),
            current_graph_context: graph_context.map(String::from),
        })
    }
}

/// Category info from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CategoryInfo {
    pub category_code: String,
    pub label: String,
    pub description: Option<String>,
    pub display_order: Option<i32>,
}

/// Workflow phase info from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowPhaseInfo {
    pub phase_code: String,
    pub label: String,
    pub description: Option<String>,
    pub phase_order: i32,
    pub transitions_to: Option<Vec<String>>,
}

/// Structured verb context for agent prompts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVerbContext {
    /// Verbs matching user intent
    pub by_intent: Vec<VerbSuggestion>,
    /// Verbs relevant to graph context
    pub by_context: Vec<VerbSuggestion>,
    /// Verbs for current workflow phase
    pub by_phase: Vec<VerbSuggestion>,
    /// Verbs in related categories
    pub by_category: Vec<VerbSuggestion>,
    /// Current workflow phase if known
    pub current_phase: Option<String>,
    /// Current graph context if known
    pub current_graph_context: Option<String>,
}

impl AgentVerbContext {
    /// Convert to prompt-friendly text
    pub fn to_prompt_text(&self) -> String {
        let mut parts = Vec::new();

        if !self.by_intent.is_empty() {
            parts.push("## Verbs Matching Your Intent".to_string());
            for s in &self.by_intent {
                let desc = s.description.as_deref().unwrap_or("");
                parts.push(format!("- `{}`: {}", s.verb, desc));
                if let Some(ref ex) = s.example {
                    parts.push(format!("  Example: `{}`", ex));
                }
            }
        }

        if !self.by_context.is_empty() {
            parts.push("\n## Verbs for Current Context".to_string());
            for s in &self.by_context {
                let desc = s.description.as_deref().unwrap_or("");
                parts.push(format!("- `{}`: {}", s.verb, desc));
            }
        }

        if !self.by_phase.is_empty() {
            if let Some(ref phase) = self.current_phase {
                parts.push(format!("\n## Verbs for {} Phase", phase));
            } else {
                parts.push("\n## Workflow Phase Verbs".to_string());
            }
            for s in &self.by_phase {
                let desc = s.description.as_deref().unwrap_or("");
                parts.push(format!("- `{}`: {}", s.verb, desc));
            }
        }

        parts.join("\n")
    }

    /// Get top N verbs across all categories
    pub fn top_verbs(&self, n: usize) -> Vec<&VerbSuggestion> {
        let mut all: Vec<&VerbSuggestion> = self
            .by_intent
            .iter()
            .chain(self.by_context.iter())
            .chain(self.by_phase.iter())
            .chain(self.by_category.iter())
            .collect();

        all.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all.truncate(n);
        all
    }

    /// Check if there are any suggestions
    pub fn is_empty(&self) -> bool {
        self.by_intent.is_empty()
            && self.by_context.is_empty()
            && self.by_phase.is_empty()
            && self.by_category.is_empty()
    }
}

/// Error type for verb discovery
#[derive(Debug, Error)]
pub enum VerbDiscoveryError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Load error: {0}")]
    LoadError(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_query_builder() {
        let query = DiscoveryQuery::new()
            .with_query("create person")
            .with_graph_context("cursor_on_cbu")
            .with_workflow_phase("intake")
            .with_limit(5);

        assert_eq!(query.query_text, Some("create person".to_string()));
        assert_eq!(query.graph_context, Some("cursor_on_cbu".to_string()));
        assert_eq!(query.workflow_phase, Some("intake".to_string()));
        assert_eq!(query.limit, 5);
    }

    #[test]
    fn test_agent_verb_context_empty() {
        let context = AgentVerbContext {
            by_intent: vec![],
            by_context: vec![],
            by_phase: vec![],
            by_category: vec![],
            current_phase: None,
            current_graph_context: None,
        };

        assert!(context.is_empty());
        assert!(context.top_verbs(5).is_empty());
    }

    #[test]
    fn test_agent_verb_context_to_prompt() {
        let context = AgentVerbContext {
            by_intent: vec![VerbSuggestion {
                verb: "entity.create-proper-person".to_string(),
                domain: "entity".to_string(),
                description: Some("Create a natural person".to_string()),
                example: Some("(entity.create-proper-person :first-name \"John\")".to_string()),
                category: Some("entity_management".to_string()),
                score: 0.95,
                reason: SuggestionReason::IntentMatch {
                    query: "create person".to_string(),
                    rank: 0.95,
                },
            }],
            by_context: vec![],
            by_phase: vec![],
            by_category: vec![],
            current_phase: None,
            current_graph_context: None,
        };

        let prompt = context.to_prompt_text();
        assert!(prompt.contains("Verbs Matching Your Intent"));
        assert!(prompt.contains("entity.create-proper-person"));
        assert!(prompt.contains("Create a natural person"));
    }
}
