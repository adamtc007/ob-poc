//! Background learning task
//!
//! Spawns after server startup to periodically:
//! 1. Run feedback analysis
//! 2. Auto-apply high-confidence patterns
//! 3. Run promotion pipeline (staged pattern promotion with quality gates)
//! 4. Check embedding coverage (warn if stale)
//!
//! Does NOT block startup - runs in background tokio task.

use anyhow::Result;
use ob_semantic_matcher::{Embedder, FeedbackService, PatternLearner, PromotionService};
use sqlx::PgPool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Status of the learning system
#[derive(Debug, Clone, Default)]
pub struct LearningStatus {
    /// Last time analysis was run
    pub last_analysis: Option<chrono::DateTime<chrono::Utc>>,
    /// Patterns applied in last run
    pub last_patterns_applied: usize,
    /// Pending embeddings count (patterns without vectors)
    pub pending_embeddings: i64,
    /// Whether embeddings are considered stale
    pub embeddings_stale: bool,
    /// Error from last run (if any)
    pub last_error: Option<String>,
}

/// Shared learning status accessible from MCP tools
pub type SharedLearningStatus = Arc<RwLock<LearningStatus>>;

/// Create shared learning status
pub fn create_learning_status() -> SharedLearningStatus {
    Arc::new(RwLock::new(LearningStatus::default()))
}

/// Configuration for the background learning task
#[derive(Debug, Clone)]
pub struct LearningConfig {
    /// Delay before first run (let server stabilize)
    pub initial_delay_secs: u64,
    /// Interval between learning runs
    pub interval_secs: u64,
    /// Days of feedback to analyze
    pub analysis_days_back: i32,
    /// Minimum occurrences to auto-apply a pattern
    pub min_occurrences: i64,
    /// Whether to run learning on startup (after delay)
    pub run_on_startup: bool,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            initial_delay_secs: 30,     // Wait 30s after startup
            interval_secs: 6 * 60 * 60, // Every 6 hours
            analysis_days_back: 7,      // Look at last 7 days
            min_occurrences: 5,         // Need 5+ occurrences to auto-apply
            run_on_startup: true,       // Check embeddings on startup
        }
    }
}

impl LearningConfig {
    /// Load from environment variables
    pub fn from_env() -> Self {
        Self {
            initial_delay_secs: std::env::var("LEARNING_INITIAL_DELAY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            interval_secs: std::env::var("LEARNING_INTERVAL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(6 * 60 * 60),
            analysis_days_back: std::env::var("LEARNING_DAYS_BACK")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(7),
            min_occurrences: std::env::var("LEARNING_MIN_OCCURRENCES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            run_on_startup: std::env::var("LEARNING_ON_STARTUP")
                .map(|s| s != "false" && s != "0")
                .unwrap_or(true),
        }
    }
}

/// Spawn background learning task
///
/// Returns immediately - task runs in background.
/// Use shutdown flag to gracefully stop the task.
pub fn spawn_learning_task(
    pool: PgPool,
    config: LearningConfig,
    status: SharedLearningStatus,
    shutdown: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        info!(
            "Learning task starting in {}s (interval: {}s)",
            config.initial_delay_secs, config.interval_secs
        );

        // Initial delay to let server stabilize
        tokio::time::sleep(Duration::from_secs(config.initial_delay_secs)).await;

        if shutdown.load(Ordering::Relaxed) {
            info!("Learning task shutdown before first run");
            return;
        }

        let feedback_service = FeedbackService::new(pool.clone());
        let pattern_learner = PatternLearner::new(pool.clone());

        // Initial check on startup
        if config.run_on_startup {
            if let Err(e) = check_embedding_coverage(&pattern_learner, &status).await {
                warn!("Initial embedding check failed: {}", e);
            }
        }

        loop {
            if shutdown.load(Ordering::Relaxed) {
                info!("Learning task shutting down");
                break;
            }

            // Run learning cycle
            match run_learning_cycle(&feedback_service, &pattern_learner, &config, &status).await {
                Ok(applied) => {
                    if applied > 0 {
                        info!("Learning cycle complete: {} patterns applied", applied);
                    }
                }
                Err(e) => {
                    error!("Learning cycle failed: {}", e);
                    let mut s = status.write().await;
                    s.last_error = Some(e.to_string());
                }
            }

            // Sleep until next interval
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(config.interval_secs)) => {}
                _ = async {
                    while !shutdown.load(Ordering::Relaxed) {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                } => {
                    info!("Learning task interrupted by shutdown");
                    break;
                }
            }
        }
    });
}

/// Run a single learning cycle
pub async fn run_learning_cycle(
    feedback_service: &FeedbackService,
    pattern_learner: &PatternLearner,
    config: &LearningConfig,
    status: &SharedLearningStatus,
) -> Result<usize> {
    info!(
        "Running learning cycle (analyzing {} days of feedback)",
        config.analysis_days_back
    );

    // 1. Run feedback analysis
    let report = feedback_service.analyze(config.analysis_days_back).await?;

    if report.is_empty() {
        info!("No patterns to learn from feedback");
        let mut s = status.write().await;
        s.last_analysis = Some(chrono::Utc::now());
        s.last_patterns_applied = 0;
        s.last_error = None;
        return Ok(0);
    }

    info!(
        "Analysis found: {} pattern discoveries, {} confusion pairs, {} gaps",
        report.pattern_discoveries.len(),
        report.confusion_pairs.len(),
        report.gaps.len()
    );

    // 2. Auto-apply high-confidence patterns (legacy path)
    let applied = pattern_learner
        .auto_apply_discoveries(&report.pattern_discoveries, config.min_occurrences)
        .await?;

    // 3. Run promotion pipeline (new staged promotion with quality gates)
    let promotion_result = run_promotion_pipeline(pattern_learner.pool()).await;
    match &promotion_result {
        Ok(report) => {
            if !report.promoted.is_empty() {
                info!(
                    "Promotion pipeline: {} patterns promoted, {} collisions, {} skipped",
                    report.promoted.len(),
                    report.collisions,
                    report.skipped
                );
            }
            if report.expired_outcomes > 0 {
                info!("Expired {} pending outcomes", report.expired_outcomes);
            }
        }
        Err(e) => {
            warn!("Promotion pipeline failed: {}", e);
        }
    }

    // 4. Check embedding coverage
    let pending = pattern_learner.count_pending_embeddings().await?;

    // 5. Update status
    let promoted_count = promotion_result
        .as_ref()
        .map(|r| r.promoted.len())
        .unwrap_or(0);
    {
        let mut s = status.write().await;
        s.last_analysis = Some(chrono::Utc::now());
        s.last_patterns_applied = applied.len() + promoted_count;
        s.pending_embeddings = pending;
        s.embeddings_stale = pending > 0;
        s.last_error = None;
    }

    if pending > 0 {
        warn!(
            "{} patterns pending embeddings. Run: cargo run --bin populate_embeddings",
            pending
        );
    }

    Ok(applied.len() + promoted_count)
}

/// Run the staged promotion pipeline
async fn run_promotion_pipeline(pool: &PgPool) -> Result<ob_semantic_matcher::PromotionReport> {
    let mut service = PromotionService::new(pool.clone());

    // Try to add embedder for collision checking
    if let Ok(embedder) = Embedder::new() {
        service = service.with_embedder(embedder);
    } else {
        warn!("Embedder not available for collision checking in promotion pipeline");
    }

    service.run_promotion_cycle().await
}

/// Check embedding coverage and update status
async fn check_embedding_coverage(
    pattern_learner: &PatternLearner,
    status: &SharedLearningStatus,
) -> Result<()> {
    let pending = pattern_learner.count_pending_embeddings().await?;

    {
        let mut s = status.write().await;
        s.pending_embeddings = pending;
        s.embeddings_stale = pending > 0;
    }

    if pending > 0 {
        warn!(
            "Embedding coverage check: {} patterns missing vectors",
            pending
        );
        warn!("Run: cargo run --release --bin populate_embeddings");
    } else {
        info!("Embedding coverage OK: all patterns have vectors");
    }

    Ok(())
}

/// Manually trigger a learning cycle (for MCP tool)
pub async fn trigger_learning_cycle(
    pool: &PgPool,
    days_back: i32,
    min_occurrences: i64,
) -> Result<LearningCycleResult> {
    let feedback_service = FeedbackService::new(pool.clone());
    let pattern_learner = PatternLearner::new(pool.clone());

    // Run analysis
    let report = feedback_service.analyze(days_back).await?;

    // Apply patterns
    let applied = pattern_learner
        .auto_apply_discoveries(&report.pattern_discoveries, min_occurrences)
        .await?;

    // Check pending
    let pending_embeddings = pattern_learner.count_pending_embeddings().await?;

    Ok(LearningCycleResult {
        patterns_discovered: report.pattern_discoveries.len(),
        confusion_pairs: report.confusion_pairs.len(),
        gaps: report.gaps.len(),
        patterns_applied: applied.len(),
        applied_patterns: applied,
        pending_embeddings,
    })
}

/// Result of a learning cycle
#[derive(Debug, Clone, serde::Serialize)]
pub struct LearningCycleResult {
    pub patterns_discovered: usize,
    pub confusion_pairs: usize,
    pub gaps: usize,
    pub patterns_applied: usize,
    pub applied_patterns: Vec<(String, String)>,
    pub pending_embeddings: i64,
}
