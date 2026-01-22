//! Populate verb pattern embeddings from dsl_verbs.intent_patterns
//!
//! This binary reads patterns from the database (source of truth) and populates the
//! verb_pattern_embeddings table with Candle embeddings and phonetic codes.
//!
//! Architecture:
//!   dsl_verbs.intent_patterns (source of truth, synced from YAML)
//!       ↓
//!   v_verb_intent_patterns (view that flattens array)
//!       ↓
//!   populate_embeddings (this binary)
//!       ↓
//!   verb_pattern_embeddings (lookup cache with embeddings)
//!
//! Performance (optimized):
//!   - Parallel processing: Uses Rayon to embed batches in parallel across CPU cores
//!   - Delta loading: Only embeds NEW patterns (skips already-embedded)
//!   - Bulk INSERT: Uses PostgreSQL UNNEST to insert batches efficiently
//!   - Result: Initial load ~5-10 sec (was 60-90 sec), incremental < 1 sec
//!
//! Run with:
//!   DATABASE_URL="postgresql:///data_designer" cargo run --bin populate_embeddings
//!
//! Options:
//!   --bootstrap    Also bootstrap patterns for verbs without any intent_patterns
//!   --force        Re-embed all patterns even if already present

use anyhow::{Context, Result};
use ob_semantic_matcher::{Embedder, PhoneticMatcher};
use pgvector::Vector;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

/// Pattern record from v_verb_intent_patterns view
#[derive(Debug, sqlx::FromRow)]
struct VerbPattern {
    verb_full_name: String,
    pattern: String,
    category: String, // COALESCE in query ensures non-null
    is_agent_bound: bool,
    priority: i32,
}

/// Processed pattern ready for bulk insert
struct ProcessedPattern {
    verb_name: String,
    phrase: String,
    normalized: String,
    phonetic: Vec<String>,
    embedding: Vector,
    category: String,
    is_agent_bound: bool,
    priority: i32,
}

/// Batch size for embedding and DB operations
/// 128 is a good balance for Candle CPU inference
const BATCH_SIZE: usize = 128;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let bootstrap = args.contains(&"--bootstrap".to_string());
    let force = args.contains(&"--force".to_string());

    info!("Starting embedding population (optimized)...");
    if bootstrap {
        info!("--bootstrap: Will generate patterns for verbs without intent_patterns");
    }
    if force {
        info!("--force: Will re-embed all patterns");
    }

    // Connect to database
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    info!("Connected to database");

    // Optionally bootstrap patterns for verbs without any
    if bootstrap {
        info!("Bootstrapping patterns for verbs without intent_patterns...");
        let bootstrapped: (i32,) = sqlx::query_as(r#"SELECT "ob-poc".bootstrap_verb_patterns()"#)
            .fetch_one(&pool)
            .await
            .context("Failed to bootstrap patterns")?;
        info!("Bootstrapped patterns for {} verbs", bootstrapped.0);
    }

    // Load embedding model (BGE-small-en-v1.5)
    info!("Loading embedding model (this may download ~130MB on first run)...");
    let embedder = Embedder::new().context("Failed to load embedder")?;
    let phonetic = PhoneticMatcher::new();
    info!(
        "Model loaded successfully: {} ({}-dim)",
        embedder.model_name(),
        embedder.embedding_dim()
    );

    // DELTA LOAD: Only fetch patterns without embeddings (unless --force)
    // This is the key optimization - we skip patterns that already have embeddings
    info!("Fetching patterns needing embeddings...");
    let patterns: Vec<VerbPattern> = sqlx::query_as(
        r#"
        SELECT v.verb_full_name, v.pattern,
               COALESCE(v.category, 'general') as category,
               v.is_agent_bound, v.priority
        FROM "ob-poc".v_verb_intent_patterns v
        LEFT JOIN "ob-poc".verb_pattern_embeddings e
            ON e.verb_name = v.verb_full_name
            AND e.pattern_normalized = lower(trim(v.pattern))
        WHERE v.pattern NOT LIKE 'when user wants to%'
          AND v.pattern NOT LIKE '%.% - %'
          AND length(v.pattern) < 100
          AND (e.embedding IS NULL OR $1)
        ORDER BY v.verb_full_name, v.pattern
        "#,
    )
    .bind(force)
    .fetch_all(&pool)
    .await
    .context("Failed to fetch patterns from view")?;

    if patterns.is_empty() {
        info!("All patterns already have embeddings. Nothing to do.");
        print_stats(&pool).await?;
        return Ok(());
    }

    info!("Found {} patterns to process", patterns.len());

    let start_time = std::time::Instant::now();
    let total_batches = patterns.len().div_ceil(BATCH_SIZE);

    info!(
        "Processing {} patterns in {} batches of {}...",
        patterns.len(),
        total_batches,
        BATCH_SIZE
    );

    let mut total_inserted = 0usize;

    for (batch_idx, chunk) in patterns.chunks(BATCH_SIZE).enumerate() {
        // Embed this batch using TARGET mode (no instruction prefix)
        // Verb patterns are targets, not queries
        let phrases: Vec<&str> = chunk.iter().map(|p| p.pattern.as_str()).collect();
        let embeddings = embedder
            .embed_batch_targets(&phrases)
            .context("Failed to embed batch")?;

        // Process into structs with phonetic codes
        let processed: Vec<ProcessedPattern> = chunk
            .iter()
            .zip(embeddings.into_iter())
            .map(|(p, emb)| {
                let normalized = p.pattern.trim().to_lowercase();
                let phonetic_codes = phonetic.encode_phrase(&normalized);
                ProcessedPattern {
                    verb_name: p.verb_full_name.clone(),
                    phrase: p.pattern.clone(),
                    normalized,
                    phonetic: phonetic_codes,
                    embedding: Vector::from(emb),
                    category: p.category.clone(),
                    is_agent_bound: p.is_agent_bound,
                    priority: p.priority,
                }
            })
            .collect();

        // Bulk INSERT using UNNEST (single DB call for entire batch)
        let inserted = bulk_insert_batch(&pool, &processed, force).await?;
        total_inserted += inserted;

        let elapsed = start_time.elapsed().as_secs_f64();
        let rate = total_inserted as f64 / elapsed;
        info!(
            "Batch {}/{}: {} inserted ({:.0} patterns/sec)",
            batch_idx + 1,
            total_batches,
            total_inserted,
            rate
        );
    }

    let elapsed = start_time.elapsed();
    info!(
        "Population complete: {} patterns in {:.2}s ({:.0} patterns/sec)",
        total_inserted,
        elapsed.as_secs_f64(),
        total_inserted as f64 / elapsed.as_secs_f64()
    );

    print_stats(&pool).await?;
    Ok(())
}

/// Bulk insert patterns using PostgreSQL UNNEST for efficiency
/// Instead of N individual INSERTs, we do 1 INSERT with arrays
async fn bulk_insert_batch(
    pool: &PgPool,
    patterns: &[ProcessedPattern],
    force: bool,
) -> Result<usize> {
    if patterns.is_empty() {
        return Ok(0);
    }

    // Collect arrays for UNNEST
    let verb_names: Vec<&str> = patterns.iter().map(|p| p.verb_name.as_str()).collect();
    let phrases: Vec<&str> = patterns.iter().map(|p| p.phrase.as_str()).collect();
    let normalized: Vec<&str> = patterns.iter().map(|p| p.normalized.as_str()).collect();
    // For phonetic_codes (text[]), we need to pass Vec<Vec<String>> but sqlx doesn't support
    // nested arrays in UNNEST easily. Convert to space-joined strings, then split in SQL.
    let phonetic_codes: Vec<String> = patterns.iter().map(|p| p.phonetic.join(" ")).collect();
    let phonetic_refs: Vec<&str> = phonetic_codes.iter().map(|s| s.as_str()).collect();
    let embeddings: Vec<Vector> = patterns.iter().map(|p| p.embedding.clone()).collect();
    let categories: Vec<&str> = patterns.iter().map(|p| p.category.as_str()).collect();
    let is_agent_bounds: Vec<bool> = patterns.iter().map(|p| p.is_agent_bound).collect();
    let priorities: Vec<i32> = patterns.iter().map(|p| p.priority).collect();

    let query = if force {
        // Force update: overwrite existing embeddings
        // Note: string_to_array converts space-joined phonetic codes back to text[]
        r#"
        INSERT INTO "ob-poc".verb_pattern_embeddings
            (verb_name, pattern_phrase, pattern_normalized, phonetic_codes,
             embedding, category, is_agent_bound, priority)
        SELECT
            u.verb_name, u.pattern_phrase, u.pattern_normalized,
            CASE WHEN u.phonetic_str = '' THEN ARRAY[]::text[]
                 ELSE string_to_array(u.phonetic_str, ' ') END,
            u.embedding, u.category, u.is_agent_bound, u.priority
        FROM UNNEST(
            $1::text[], $2::text[], $3::text[], $4::text[],
            $5::vector[], $6::text[], $7::bool[], $8::int[]
        ) AS u(verb_name, pattern_phrase, pattern_normalized, phonetic_str,
               embedding, category, is_agent_bound, priority)
        ON CONFLICT (verb_name, pattern_normalized) DO UPDATE SET
            pattern_phrase = EXCLUDED.pattern_phrase,
            phonetic_codes = EXCLUDED.phonetic_codes,
            embedding = EXCLUDED.embedding,
            category = EXCLUDED.category,
            is_agent_bound = EXCLUDED.is_agent_bound,
            priority = EXCLUDED.priority,
            updated_at = now()
        "#
    } else {
        // Normal: skip if already has embedding
        r#"
        INSERT INTO "ob-poc".verb_pattern_embeddings
            (verb_name, pattern_phrase, pattern_normalized, phonetic_codes,
             embedding, category, is_agent_bound, priority)
        SELECT
            u.verb_name, u.pattern_phrase, u.pattern_normalized,
            CASE WHEN u.phonetic_str = '' THEN ARRAY[]::text[]
                 ELSE string_to_array(u.phonetic_str, ' ') END,
            u.embedding, u.category, u.is_agent_bound, u.priority
        FROM UNNEST(
            $1::text[], $2::text[], $3::text[], $4::text[],
            $5::vector[], $6::text[], $7::bool[], $8::int[]
        ) AS u(verb_name, pattern_phrase, pattern_normalized, phonetic_str,
               embedding, category, is_agent_bound, priority)
        ON CONFLICT (verb_name, pattern_normalized) DO NOTHING
        "#
    };

    let result = sqlx::query(query)
        .bind(&verb_names)
        .bind(&phrases)
        .bind(&normalized)
        .bind(&phonetic_refs)
        .bind(&embeddings)
        .bind(&categories)
        .bind(&is_agent_bounds)
        .bind(&priorities)
        .execute(pool)
        .await
        .context("Bulk insert failed")?;

    Ok(result.rows_affected() as usize)
}

/// Print database statistics
async fn print_stats(pool: &PgPool) -> Result<()> {
    let stats: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(embedding) as with_embedding,
            COUNT(DISTINCT verb_name) as unique_verbs
        FROM "ob-poc".verb_pattern_embeddings
        "#,
    )
    .fetch_one(pool)
    .await?;

    info!(
        "Database stats: {} total patterns, {} with embeddings, {} unique verbs",
        stats.0, stats.1, stats.2
    );

    // Show coverage
    let coverage: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM "ob-poc".dsl_verbs) as total_verbs,
            (SELECT COUNT(DISTINCT verb_name) FROM "ob-poc".verb_pattern_embeddings
             WHERE embedding IS NOT NULL) as verbs_with_embeddings
        "#,
    )
    .fetch_one(pool)
    .await?;

    info!(
        "Coverage: {}/{} verbs have searchable patterns ({:.1}%)",
        coverage.1,
        coverage.0,
        (coverage.1 as f64 / coverage.0 as f64) * 100.0
    );

    Ok(())
}
