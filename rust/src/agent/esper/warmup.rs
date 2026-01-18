//! ESPER command warmup - loads config + learned aliases at startup
//!
//! Also pre-computes embeddings for semantic fallback (Phase 8).

use super::config::EsperConfig;
use super::registry::{EsperCommandRegistry, SemanticIndex};
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

#[cfg(feature = "database")]
use sqlx::PgPool;

use crate::agent::learning::embedder::CandleEmbedder;

/// Statistics from ESPER warmup
#[derive(Debug, Clone)]
pub struct EsperWarmupStats {
    /// Number of commands loaded from YAML
    pub command_count: usize,
    /// Number of learned aliases loaded from DB
    pub learned_count: usize,
    /// Number of aliases embedded for semantic search
    pub embedded_count: usize,
    /// Time taken for warmup in milliseconds
    pub warmup_ms: u64,
    /// Time taken for embedding computation in milliseconds
    pub embed_ms: u64,
}

/// ESPER warmup handler
pub struct EsperWarmup {
    config_path: std::path::PathBuf,
    #[cfg(feature = "database")]
    pool: Option<PgPool>,
    /// Embedder for semantic index (optional - skips semantic if None)
    embedder: Option<Arc<CandleEmbedder>>,
}

impl EsperWarmup {
    /// Create warmup handler with config path only (no DB, no embedder)
    pub fn new<P: AsRef<Path>>(config_path: P) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            #[cfg(feature = "database")]
            pool: None,
            embedder: None,
        }
    }

    /// Create warmup handler with optional DB pool
    #[cfg(feature = "database")]
    pub fn with_pool<P: AsRef<Path>>(config_path: P, pool: PgPool) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            pool: Some(pool),
            embedder: None,
        }
    }

    /// Create warmup handler from optional pool (uses default config path)
    #[cfg(feature = "database")]
    pub fn from_pool(pool: Option<PgPool>) -> Self {
        Self {
            config_path: default_config_path(),
            pool,
            embedder: None,
        }
    }

    /// Set embedder for semantic index computation
    pub fn with_embedder(mut self, embedder: Arc<CandleEmbedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Load YAML config only (for listing commands, etc.)
    pub fn load_config(&self) -> Result<EsperConfig> {
        EsperConfig::load(&self.config_path)
    }

    /// Load YAML config + learned aliases, build registry with semantic index
    pub async fn warmup(&self) -> Result<(EsperCommandRegistry, EsperWarmupStats)> {
        let start = std::time::Instant::now();

        // 1. Load YAML config
        let config = match EsperConfig::load(&self.config_path) {
            Ok(c) => {
                info!(
                    "Loaded {} ESPER commands from {:?}",
                    c.commands.len(),
                    self.config_path
                );
                c
            }
            Err(e) => {
                warn!(
                    "Failed to load ESPER config from {:?}: {}",
                    self.config_path, e
                );
                return Err(e);
            }
        };

        // 2. Load approved aliases from DB (if available)
        #[cfg(feature = "database")]
        let learned = if let Some(pool) = &self.pool {
            self.load_learned_aliases(pool).await?
        } else {
            HashMap::new()
        };

        #[cfg(not(feature = "database"))]
        let learned = HashMap::new();

        let learned_count = learned.len();
        if learned_count > 0 {
            info!(
                "Loaded {} learned ESPER aliases from database",
                learned_count
            );
        }

        // 3. Build registry
        let command_count = config.commands.len();
        let mut registry = EsperCommandRegistry::new(config, learned);

        // 4. Build semantic index if embedder available (Phase 8)
        let (embedded_count, embed_ms) = if let Some(embedder) = &self.embedder {
            let embed_start = std::time::Instant::now();
            match self.build_semantic_index(&registry, embedder) {
                Ok(index) => {
                    let count = index.len();
                    registry.set_semantic_index(index);
                    let ms = embed_start.elapsed().as_millis() as u64;
                    info!(
                        "Built ESPER semantic index with {} embeddings in {}ms",
                        count, ms
                    );
                    (count, ms)
                }
                Err(e) => {
                    warn!("Failed to build ESPER semantic index: {}", e);
                    (0, 0)
                }
            }
        } else {
            (0, 0)
        };

        let warmup_ms = start.elapsed().as_millis() as u64;
        info!("ESPER warmup complete in {}ms", warmup_ms);

        let stats = EsperWarmupStats {
            command_count,
            learned_count,
            embedded_count,
            warmup_ms,
            embed_ms,
        };

        Ok((registry, stats))
    }

    /// Build semantic index by computing embeddings for all aliases
    fn build_semantic_index(
        &self,
        registry: &EsperCommandRegistry,
        embedder: &CandleEmbedder,
    ) -> Result<SemanticIndex> {
        let aliases = registry.all_aliases();
        if aliases.is_empty() {
            return Ok(SemanticIndex::new());
        }

        // Extract just the alias texts for batch embedding
        let texts: Vec<&str> = aliases.iter().map(|(text, _)| text.as_str()).collect();

        // Batch embed all aliases (blocking - this runs at startup)
        let embeddings = embedder.embed_batch_blocking(&texts)?;

        // Build the index
        let mut index = SemanticIndex::new();
        for ((alias, command_key), embedding) in aliases.into_iter().zip(embeddings) {
            index.add_alias(alias, command_key, embedding);
        }
        index.mark_ready();

        Ok(index)
    }

    /// Load only from YAML (no DB, for testing)
    pub fn warmup_sync(&self) -> Result<EsperCommandRegistry> {
        let config = EsperConfig::load(&self.config_path)?;
        Ok(EsperCommandRegistry::new(config, HashMap::new()))
    }

    #[cfg(feature = "database")]
    async fn load_learned_aliases(&self, pool: &PgPool) -> Result<HashMap<String, String>> {
        let rows = sqlx::query!(
            r#"
            SELECT phrase, command_key
            FROM agent.esper_aliases
            WHERE auto_approved = true
            ORDER BY occurrence_count DESC
            "#
        )
        .fetch_all(pool)
        .await?;

        let mut learned = HashMap::new();
        for row in rows {
            learned.insert(row.phrase, row.command_key);
        }

        Ok(learned)
    }
}

/// Load ESPER config from default location
#[allow(dead_code)]
pub fn default_config_path() -> std::path::PathBuf {
    // Check DSL_CONFIG_DIR env var first
    if let Ok(dir) = std::env::var("DSL_CONFIG_DIR") {
        return std::path::PathBuf::from(dir).join("esper-commands.yaml");
    }

    // Fall back to relative path from crate root
    std::path::PathBuf::from("config/esper-commands.yaml")
}
