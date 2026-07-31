//! Startup Learning Warmup
//!
//! Loads learned data at server startup and applies pending learnings.
//! This ensures the agent benefits from accumulated knowledge immediately.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::inspector::AgentLearningInspector;

/// Statistics from warmup process.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct WarmupStats {
    /// Entity aliases loaded
    pub entity_aliases_loaded: usize,
    /// Lexicon tokens loaded
    pub lexicon_tokens_loaded: usize,
    /// Invocation phrases loaded
    pub invocation_phrases_loaded: usize,
    /// Pending learnings auto-applied
    pub learnings_auto_applied: usize,
    /// Time taken in milliseconds
    pub duration_ms: u64,
}

/// In-memory learned data for fast lookup.
#[derive(Debug, Default)]
pub(crate) struct LearnedData {
    /// Entity aliases: lowercase alias → (canonical_name, entity_id)
    pub entity_aliases: HashMap<String, (String, Option<Uuid>)>,
    /// Lexicon tokens: lowercase token → (token_type, subtype)
    pub lexicon_tokens: HashMap<String, (String, Option<String>)>,
    /// Invocation phrases: lowercase phrase → verb
    pub invocation_phrases: HashMap<String, String>,
}

/// Shared learned data for use across requests.
pub(crate) type SharedLearnedData = Arc<RwLock<LearnedData>>;

/// Learning warmup handler.
pub struct LearningWarmup {
    pool: PgPool,
}

impl LearningWarmup {
    /// Create new warmup handler.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Execute full warmup: load data + apply pending learnings.
    ///
    /// Call this at server startup. Blocks until complete.
    /// Typical duration: 100-200ms.
    pub async fn warmup(&self) -> Result<(SharedLearnedData, WarmupStats)> {
        let start = std::time::Instant::now();
        let mut stats = WarmupStats::default();

        let inspector = AgentLearningInspector::new(self.pool.clone());

        // Phase 1: Apply pending threshold-based learnings
        let applied = inspector.apply_threshold_learnings().await.unwrap_or(0);
        stats.learnings_auto_applied = applied;

        // Phase 2: Load all learned data into memory
        let mut data = LearnedData::default();

        // Load entity aliases
        if let Ok(aliases) = inspector.get_entity_aliases().await {
            stats.entity_aliases_loaded = aliases.len();
            for (alias, canonical, entity_id) in aliases {
                data.entity_aliases
                    .insert(alias.to_lowercase(), (canonical, entity_id));
            }
        }

        // Load lexicon tokens
        if let Ok(tokens) = inspector.get_lexicon_tokens().await {
            stats.lexicon_tokens_loaded = tokens.len();
            for (token, token_type, subtype) in tokens {
                data.lexicon_tokens
                    .insert(token.to_lowercase(), (token_type, subtype));
            }
        }

        // Load invocation phrases
        if let Ok(phrases) = inspector.get_invocation_phrases().await {
            stats.invocation_phrases_loaded = phrases.len();
            for (phrase, verb) in phrases {
                data.invocation_phrases.insert(phrase.to_lowercase(), verb);
            }
        }

        stats.duration_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            aliases = stats.entity_aliases_loaded,
            tokens = stats.lexicon_tokens_loaded,
            phrases = stats.invocation_phrases_loaded,
            auto_applied = stats.learnings_auto_applied,
            duration_ms = stats.duration_ms,
            "Agent learning warmup complete"
        );

        Ok((Arc::new(RwLock::new(data)), stats))
    }

    /// Reload learned data (e.g., after manual approval).
    ///
    /// Can be called without restarting server.
    pub async fn reload(&self, data: &SharedLearnedData) -> Result<WarmupStats> {
        let start = std::time::Instant::now();
        let mut stats = WarmupStats::default();

        let inspector = AgentLearningInspector::new(self.pool.clone());

        let mut new_data = LearnedData::default();

        // Load entity aliases
        if let Ok(aliases) = inspector.get_entity_aliases().await {
            stats.entity_aliases_loaded = aliases.len();
            for (alias, canonical, entity_id) in aliases {
                new_data
                    .entity_aliases
                    .insert(alias.to_lowercase(), (canonical, entity_id));
            }
        }

        // Load lexicon tokens
        if let Ok(tokens) = inspector.get_lexicon_tokens().await {
            stats.lexicon_tokens_loaded = tokens.len();
            for (token, token_type, subtype) in tokens {
                new_data
                    .lexicon_tokens
                    .insert(token.to_lowercase(), (token_type, subtype));
            }
        }

        // Load invocation phrases
        if let Ok(phrases) = inspector.get_invocation_phrases().await {
            stats.invocation_phrases_loaded = phrases.len();
            for (phrase, verb) in phrases {
                new_data
                    .invocation_phrases
                    .insert(phrase.to_lowercase(), verb);
            }
        }

        // Swap in new data
        let mut write_guard = data.write().await;
        *write_guard = new_data;

        stats.duration_ms = start.elapsed().as_millis() as u64;

        tracing::info!(
            aliases = stats.entity_aliases_loaded,
            tokens = stats.lexicon_tokens_loaded,
            phrases = stats.invocation_phrases_loaded,
            duration_ms = stats.duration_ms,
            "Agent learning data reloaded"
        );

        Ok(stats)
    }
}

impl LearnedData {
    /// Look up invocation phrase.
    pub(crate) fn resolve_phrase(&self, phrase: &str) -> Option<&str> {
        self.invocation_phrases
            .get(&phrase.to_lowercase())
            .map(|v| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learned_data_lookups() {
        let mut data = LearnedData::default();

        data.entity_aliases.insert(
            "barclays".to_string(),
            ("Barclays PLC".to_string(), Some(Uuid::new_v4())),
        );
        data.lexicon_tokens.insert(
            "counterparty".to_string(),
            ("Entity".to_string(), Some("Counterparty".to_string())),
        );
        data.invocation_phrases
            .insert("set up isda".to_string(), "isda.create".to_string());

        // Case-insensitive lookup
        assert!(data.resolve_phrase("Set Up ISDA").is_some());
    }
}
